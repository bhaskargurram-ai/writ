//! `writ integrate`: wire writ into an agent's own configuration.
//!
//! `writ integrate claude-code` merges writ's hook entries into the
//! project's `.claude/settings.json` (current directory):
//!
//! - every other setting, and every hook that is not writ's, is preserved
//!   (key order included);
//! - writ's own entries — command hooks that run a `writ` executable's
//!   `check --format claude-code` — are replaced, never duplicated, so
//!   running it twice changes nothing and re-running after moving the
//!   binary updates the paths;
//! - paths are absolute: the running writ executable
//!   (`std::env::current_exe`), the policy and the ledger;
//! - the policy must exist and compile first — a hook pointing at a missing
//!   policy would deny every tool call;
//! - the file is replaced atomically (write to a sibling temp file, then
//!   rename); a settings file that is not a JSON object is refused rather
//!   than overwritten.
//!
//! `--print` prints the merged settings instead of writing them.

use std::path::{Path, PathBuf};

use anyhow::{anyhow, bail, Context, Result};
use serde_json::{Map, Value};

use crate::cmds::load_engine;

/// Agents `writ integrate` knows how to configure.
#[derive(Clone, Copy, Debug, PartialEq, Eq, clap::ValueEnum)]
pub enum Target {
    /// Claude Code hooks in `.claude/settings.json`.
    ClaudeCode,
}

pub fn integrate(policy: &Path, ledger: &Path, target: Target, print: bool) -> Result<()> {
    match target {
        Target::ClaudeCode => integrate_claude_code(policy, ledger, print),
    }
}

fn integrate_claude_code(policy: &Path, ledger: &Path, print: bool) -> Result<()> {
    let policy = std::path::absolute(policy).context("resolve --policy")?;
    let ledger = std::path::absolute(ledger).context("resolve --ledger")?;
    load_engine(&policy, false).with_context(|| {
        format!(
            "refusing to integrate: the hooks would deny every tool call until {} loads",
            policy.display()
        )
    })?;
    let exe = std::env::current_exe().context("locate the running writ executable")?;
    let fragment = claude_code_hook_settings(&exe, &policy, &ledger);

    let settings_path = PathBuf::from(".claude").join("settings.json");
    let existing = match std::fs::read_to_string(&settings_path) {
        Ok(s) if s.trim().is_empty() => Value::Object(Map::new()),
        Ok(s) => serde_json::from_str::<Value>(&s).with_context(|| {
            format!(
                "{} is not valid JSON; not touching it",
                settings_path.display()
            )
        })?,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Value::Object(Map::new()),
        Err(e) => return Err(e).with_context(|| format!("read {}", settings_path.display())),
    };
    let merged = merge_hook_settings(existing, &fragment)?;
    let text = serde_json::to_string_pretty(&merged)? + "\n";

    if print {
        print!("{text}");
        return Ok(());
    }
    std::fs::create_dir_all(".claude").context("create .claude/")?;
    let tmp = settings_path.with_extension(format!("json.writ-{}.tmp", std::process::id()));
    std::fs::write(&tmp, &text).with_context(|| format!("write {}", tmp.display()))?;
    if let Err(e) = std::fs::rename(&tmp, &settings_path) {
        let _ = std::fs::remove_file(&tmp);
        return Err(e).with_context(|| format!("replace {}", settings_path.display()));
    }
    eprintln!(
        "  writ · Claude Code hooks written to {} (PreToolUse, PostToolUse, PostToolUseFailure)\n  policy: {}\n  ledger: {}",
        settings_path.display(),
        policy.display(),
        ledger.display()
    );
    Ok(())
}

/// Merge `fragment` (`{"hooks": {event: [groups]}}`) into `settings`,
/// replacing writ's own previous hook entries and keeping everything else.
pub(crate) fn merge_hook_settings(mut settings: Value, fragment: &Value) -> Result<Value> {
    let root = settings
        .as_object_mut()
        .ok_or_else(|| anyhow!("Claude Code settings must be a JSON object; not touching it"))?;
    let hooks = root
        .entry("hooks")
        .or_insert_with(|| Value::Object(Map::new()));
    let hooks = hooks.as_object_mut().ok_or_else(|| {
        anyhow!("\"hooks\" in Claude Code settings is not an object; not touching it")
    })?;
    let Some(new_hooks) = fragment.get("hooks").and_then(Value::as_object) else {
        bail!("internal: hook fragment has no \"hooks\" object");
    };
    for (event, new_groups) in new_hooks {
        let groups = hooks
            .entry(event.clone())
            .or_insert_with(|| Value::Array(Vec::new()));
        let groups = groups.as_array_mut().ok_or_else(|| {
            anyhow!("\"hooks.{event}\" in Claude Code settings is not an array; not touching it")
        })?;
        // Drop writ's previous handlers; drop groups left empty by that.
        groups.retain_mut(|group| {
            let Some(list) = group.get_mut("hooks").and_then(Value::as_array_mut) else {
                return true;
            };
            let before = list.len();
            list.retain(|h| !is_writ_hook(h));
            !(before > 0 && list.is_empty())
        });
        if let Some(arr) = new_groups.as_array() {
            groups.extend(arr.iter().cloned());
        }
    }
    Ok(settings)
}

/// A command hook that runs writ's Claude Code gateway: exec form with a
/// `writ` executable and `check`/`claude-code` in its args, or a shell-form
/// command line containing `check --format claude-code`.
fn is_writ_hook(h: &Value) -> bool {
    let Some(cmd) = h.get("command").and_then(Value::as_str) else {
        return false;
    };
    if let Some(args) = h.get("args").and_then(Value::as_array) {
        let stem = Path::new(cmd)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("");
        let has = |a: &str| args.iter().any(|x| x.as_str() == Some(a));
        return stem.eq_ignore_ascii_case("writ") && has("check") && has("claude-code");
    }
    cmd.contains("check --format claude-code")
}

/// The Claude Code settings fragment (`{"hooks": {...}}`) that routes every
/// tool call through `writ check --format claude-code`. Shared by
/// `writ integrate claude-code` and `writ run -- claude` (which passes it to
/// Claude Code with `--settings`). `writ_exe` is the absolute path of the
/// writ binary; `policy`/`ledger` are passed through as absolute paths.
///
/// Exec form (`command` + `args`, no shell) so paths with spaces, quotes or
/// backslashes need no quoting on any platform. `PostToolUseFailure` records
/// the execution of tools that failed. `--ask defer` turns a writ `ask` into
/// Claude Code's own permission prompt.
#[allow(dead_code)] // also called by `writ run`
pub(crate) fn claude_code_hook_settings(
    writ_exe: &Path,
    policy: &Path,
    ledger: &Path,
) -> serde_json::Value {
    let hook = serde_json::json!([{
        "matcher": "*",
        "hooks": [{
            "type": "command",
            "command": writ_exe.display().to_string(),
            "args": [
                "--policy", policy.display().to_string(),
                "--ledger", ledger.display().to_string(),
                "check", "--format", "claude-code", "--ask", "defer"
            ]
        }]
    }]);
    serde_json::json!({ "hooks": {
        "PreToolUse": hook,
        "PostToolUse": hook,
        "PostToolUseFailure": hook,
    } })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn frag(exe: &str) -> Value {
        claude_code_hook_settings(
            Path::new(exe),
            Path::new("/p/writ.yaml"),
            Path::new("/p/l.jsonl"),
        )
    }

    #[test]
    fn merge_preserves_and_is_idempotent() {
        let existing = json!({
            "model": "opus",
            "permissions": {"allow": ["Bash(ls)"]},
            "hooks": {
                "PreToolUse": [{"matcher": "Bash", "hooks": [{"type": "command", "command": "lint.sh"}]}],
                "Stop": [{"hooks": [{"type": "command", "command": "notify"}]}]
            }
        });
        let once = merge_hook_settings(existing.clone(), &frag("/bin/writ")).unwrap();
        let twice = merge_hook_settings(once.clone(), &frag("/bin/writ")).unwrap();
        assert_eq!(once, twice);
        assert_eq!(once["model"], "opus");
        assert_eq!(once["hooks"]["Stop"], existing["hooks"]["Stop"]);
        let pre = once["hooks"]["PreToolUse"].as_array().unwrap();
        assert_eq!(pre.len(), 2);
        assert_eq!(pre[0]["hooks"][0]["command"], "lint.sh");
        assert_eq!(pre[1]["hooks"][0]["command"], "/bin/writ");

        // Moving the binary replaces the entry instead of adding one.
        let moved = merge_hook_settings(once, &frag("/opt/writ.exe")).unwrap();
        let pre = moved["hooks"]["PreToolUse"].as_array().unwrap();
        assert_eq!(pre.len(), 2);
        assert_eq!(pre[1]["hooks"][0]["command"], "/opt/writ.exe");
    }

    #[test]
    fn old_shell_form_entry_is_replaced() {
        let existing = json!({"hooks": {"PreToolUse": [{"matcher": "*", "hooks": [
            {"type": "command", "command": "\"/x/writ\" --policy \"a\" --ledger \"b\" check --format claude-code --ask defer"}
        ]}]}});
        let merged = merge_hook_settings(existing, &frag("/bin/writ")).unwrap();
        assert_eq!(merged["hooks"]["PreToolUse"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn refuses_non_object_settings() {
        assert!(merge_hook_settings(json!([1]), &frag("/bin/writ")).is_err());
        assert!(merge_hook_settings(json!({"hooks": 3}), &frag("/bin/writ")).is_err());
    }
}
