//! writ.yaml loading: serde structs, compilation (regexes, list refs,
//! durations), and per-rule source line tracking.
//!
//! Every error message carries `writ.yaml:LINE` detail (spec §7/§12) so a
//! failed `reload` tells the operator exactly where the policy broke.

use crate::ast::{Expr, Op, Predicate, RawExpr, RawOp, RawPredicate};
use crate::parser::parse_when;
use regex::Regex;
use serde::Deserialize;
use std::collections::HashMap;
use writ_core::verdict::DefaultVerdict;

/// The display name used in all diagnostics and verdict locations.
pub const POLICY_FILE_NAME: &str = "writ.yaml";

#[derive(Debug, Deserialize)]
struct RawPolicyFile {
    version: u32,
    default: DefaultVerdict,
    #[serde(default)]
    rules: Vec<RawRule>,
    /// Any other top-level mapping (e.g. `hosts: { allowed: [...] }`) is a
    /// namespace of named lists addressable from `in` expressions.
    #[serde(flatten)]
    lists: HashMap<String, serde_yaml::Value>,
}

#[derive(Debug, Deserialize)]
struct RawRule {
    id: String,
    when: String,
    verdict: String,
    #[serde(default)]
    reason: Option<String>,
    #[serde(default)]
    irreversible: bool,
    #[serde(default)]
    timeout: Option<String>,
    #[serde(default)]
    patterns: Option<Vec<String>>,
}

/// The four rule verdicts of the native engine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuleVerdict {
    Allow,
    Deny,
    Ask,
    Redact,
}

/// A fully validated, evaluation-ready rule.
#[derive(Debug, Clone)]
pub struct CompiledRule {
    pub id: String,
    pub when: Expr,
    pub verdict: RuleVerdict,
    pub reason: Option<String>,
    pub irreversible: bool,
    pub timeout_ms: Option<u64>,
    pub patterns: Vec<String>,
    /// 1-based line of `- id: <id>` in the source, if found.
    pub line: Option<usize>,
}

/// A fully validated, evaluation-ready policy.
#[derive(Debug, Clone)]
pub struct CompiledPolicy {
    pub version: u32,
    pub default: DefaultVerdict,
    pub rules: Vec<CompiledRule>,
}

/// Parse `"30s"`, `"5m"`, `"1h"`, `"250ms"` (or a bare integer = milliseconds)
/// into milliseconds.
pub fn parse_duration(s: &str) -> Result<u64, String> {
    let s = s.trim();
    let (num, mult) = if let Some(n) = s.strip_suffix("ms") {
        (n, 1u64)
    } else if let Some(n) = s.strip_suffix('s') {
        (n, 1_000)
    } else if let Some(n) = s.strip_suffix('m') {
        (n, 60_000)
    } else if let Some(n) = s.strip_suffix('h') {
        (n, 3_600_000)
    } else {
        (s, 1)
    };
    let v: u64 = num
        .trim()
        .parse()
        .map_err(|_| format!("invalid duration {:?} (expected e.g. \"30s\", \"5m\", \"1h\")", s))?;
    v.checked_mul(mult)
        .ok_or_else(|| format!("duration {:?} overflows milliseconds", s))
}


/// Parse and compile a writ.yaml source. Pure: on `Err` nothing is mutated —
/// callers implement atomic reload by only swapping on `Ok`.
pub fn compile(source: &str) -> Result<CompiledPolicy, String> {
    let raw: RawPolicyFile = serde_yaml::from_str(source).map_err(|e| match e.location() {
        Some(loc) => format!("{}:{}:{}: {}", POLICY_FILE_NAME, loc.line(), loc.column(), e),
        None => format!("{}: {}", POLICY_FILE_NAME, e),
    })?;

    // Flatten named lists: "hosts.allowed" -> ["api.github.com", ...].
    let mut lists: HashMap<String, Vec<String>> = HashMap::new();
    for (ns, value) in &raw.lists {
        let mapping = value.as_mapping().ok_or_else(|| {
            format!(
                "{}: top-level key {:?} must be a mapping of list names to string lists",
                POLICY_FILE_NAME, ns
            )
        })?;
        for (k, v) in mapping {
            let list_name = k.as_str().ok_or_else(|| {
                format!("{}: list names under {:?} must be strings", POLICY_FILE_NAME, ns)
            })?;
            let seq = v.as_sequence().ok_or_else(|| {
                format!("{}: list {:?} must be a sequence of strings", POLICY_FILE_NAME, list_name)
            })?;
            let mut items = Vec::with_capacity(seq.len());
            for item in seq {
                let s = item.as_str().ok_or_else(|| {
                    format!(
                        "{}: entries of list {:?} must be strings, got {:?}",
                        POLICY_FILE_NAME, list_name, item
                    )
                })?;
                items.push(s.to_string());
            }
            lists.insert(format!("{}.{}", ns, list_name), items);
        }
    }

    // Track each rule's `- id: <id>` source line.
    let id_lines = extract_rule_id_lines(source);
    let mut cursor = 0usize;
    let mut rules = Vec::with_capacity(raw.rules.len());
    for (i, rr) in raw.rules.iter().enumerate() {
        let line = locate_rule_line(&id_lines, &rr.id, i, &mut cursor);
        let at = |msg: String| match line {
            Some(l) => format!("{}:{}: rule {:?}: {}", POLICY_FILE_NAME, l, rr.id, msg),
            None => format!("{}: rule {:?}: {}", POLICY_FILE_NAME, rr.id, msg),
        };

        if rr.id.trim().is_empty() {
            return Err(at("rule id must not be empty".to_string()));
        }
        let raw_expr = parse_when(&rr.when).map_err(|e| at(format!("invalid `when`: {}", e)))?;
        let when = compile_expr(&raw_expr, &lists).map_err(&at)?;
        let verdict = match rr.verdict.as_str() {
            "allow" => RuleVerdict::Allow,
            "deny" => RuleVerdict::Deny,
            "ask" => RuleVerdict::Ask,
            "redact" => RuleVerdict::Redact,
            other => {
                return Err(at(format!(
                    "unknown verdict {:?} (expected allow, deny, ask, redact)",
                    other
                )))
            }
        };
        let timeout_ms = rr
            .timeout
            .as_deref()
            .map(parse_duration)
            .transpose()
            .map_err(&at)?;
        let patterns = rr.patterns.clone().unwrap_or_default();
        if verdict == RuleVerdict::Redact && patterns.is_empty() {
            return Err(at("`verdict: redact` requires a non-empty `patterns` list".to_string()));
        }
        rules.push(CompiledRule {
            id: rr.id.clone(),
            when,
            verdict,
            reason: rr.reason.clone(),
            irreversible: rr.irreversible,
            timeout_ms,
            patterns,
            line,
        });
    }

/// Compile a raw expression: build regexes and resolve `in` list refs.
fn compile_expr(raw: &RawExpr, lists: &HashMap<String, Vec<String>>) -> Result<Expr, String> {
    Ok(match raw {
        RawExpr::Or(a, b) => Expr::Or(
            Box::new(compile_expr(a, lists)?),
            Box::new(compile_expr(b, lists)?),
        ),
        RawExpr::And(a, b) => Expr::And(
            Box::new(compile_expr(a, lists)?),
            Box::new(compile_expr(b, lists)?),
        ),
        RawExpr::Not(inner) => Expr::Not(Box::new(compile_expr(inner, lists)?)),
        RawExpr::Pred(RawPredicate::Compare { field, op }) => Expr::Pred(Predicate::Compare {
            field: *field,
            op: compile_op(op)?,
        }),
        RawExpr::Pred(RawPredicate::In { field, list_ref }) => {
            let list = lists.get(list_ref).cloned().ok_or_else(|| {
                let mut known: Vec<&str> = lists.keys().map(String::as_str).collect();
                known.sort();
                format!(
                    "unknown list reference {:?} (defined lists: {})",
                    list_ref,
                    if known.is_empty() { "<none>".to_string() } else { known.join(", ") }
                )
            })?;
            Expr::Pred(Predicate::In { field: *field, list })
        }
    })
}

fn compile_op(op: &RawOp) -> Result<Op, String> {
    Ok(match op {
        RawOp::Eq(v) => Op::Eq(v.clone()),
        RawOp::Ne(v) => Op::Ne(v.clone()),
        RawOp::StartsWith(v) => Op::StartsWith(v.clone()),
        RawOp::EndsWith(v) => Op::EndsWith(v.clone()),
        RawOp::Contains(v) => Op::Contains(v.clone()),
        RawOp::Matches(pat) => Op::Matches(
            Regex::new(pat).map_err(|e| format!("invalid regex {:?}: {}", pat, e))?,
        ),
    })
}

/// All lines (1-based) of sequence entries declaring an id: `- id: <value>`.
fn extract_rule_id_lines(source: &str) -> Vec<(usize, String)> {
    let re = Regex::new(r#"^\s*-\s*id\s*:\s*(.+?)\s*(?:#.*)?$"#).expect("static regex");
    source
        .lines()
        .enumerate()
        .filter_map(|(i, line)| {
            re.captures(line).map(|c| {
                let id = c[1].trim().trim_matches('"').trim_matches('\'').to_string();
                (i + 1, id)
            })
        })
        .collect()
}

/// Match rules to source lines in order of appearance. Falls back to the
/// nth `- id:` line if an exact id match cannot be found ahead of the cursor.
fn locate_rule_line(
    id_lines: &[(usize, String)],
    id: &str,
    index: usize,
    cursor: &mut usize,
) -> Option<usize> {
    if let Some(pos) = id_lines[*cursor..].iter().position(|(_, v)| v == id) {
        let abs = *cursor + pos;
        *cursor = abs + 1;
        return Some(id_lines[abs].0);
    }
    if let Some(pos) = id_lines.iter().position(|(_, v)| v == id) {
        return Some(id_lines[pos].0);
    }
    id_lines.get(index).map(|(l, _)| *l)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_durations() {
        assert_eq!(parse_duration("30s").unwrap(), 30_000);
        assert_eq!(parse_duration("5m").unwrap(), 300_000);
        assert_eq!(parse_duration("1h").unwrap(), 3_600_000);
        assert_eq!(parse_duration("250ms").unwrap(), 250);
        assert_eq!(parse_duration("500").unwrap(), 500);
        assert!(parse_duration("5d").is_err());
        assert!(parse_duration("soon").is_err());
    }
}


    Ok(CompiledPolicy { version: raw.version, default: raw.default, rules })
}
