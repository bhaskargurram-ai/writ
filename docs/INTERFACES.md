# INTERFACES.md — Frozen Contracts (Wave 0)

**Status: FROZEN.** Changes require an ADR in `docs/DECISIONS.md` and A0 sign-off.
The source of truth is code in `crates/writ-core`; this document is the map.

## Contract 1 — `ToolCall` envelope (`writ_core::call`)

Every interceptor (MCP proxy / process wrap / SDK hook) normalizes to `ToolCall`:
`call_id, session_id, caller: CallerIdentity, mode: InterceptMode, tool, args: Value, server: Option<ServerIdentity>, trust: Option<TrustVerdict>, captured_at`.

- Credentials NEVER appear in `args` — writ-mcp injects them at dispatch, after the ledger write.
- `ToolCallContext::from_call(&ToolCall)` produces the policy-evaluation vocabulary: `tool`, `command`, `path`, `url_host`, `query`, `server`, `trust`, `mode`, `agent`. DSL identifiers map: `url.host` → `url_host`, `server.trust` → `trust`.

## Contract 2 — `PolicyEngine` + `Verdict` IR (`writ_core::policy`, `writ_core::verdict`)

```rust
trait PolicyEngine { name(); evaluate(&ToolCallContext) -> Verdict; reload(&mut self, &str) -> Result<()>; rule_count(); }
```

`Verdict = Allow{rule_id?} | Deny{rule_id, reason, location?} | Ask{rule_id, diff, timeout_ms?, irreversible, location?} | Redact{rule_id, patterns}`.

- Engines always return a verdict; unmatched calls hit the policy's `default` (spec §7: `ask`, fail-closed; `--yolo` flips to `allow` at the CLI layer).
- `reload` is atomic: parse failure keeps the last-good policy.
- All engines (native, Rego, Cedar) must agree on `crates/writ-policy/fixtures/`.
- Denials MUST carry `rule_id`, human `reason`, and `location` (`writ.yaml:LINE`) — never "denied by policy" (spec §12).

## Contract 3 — `LedgerRecord` schema v1 (`writ_core::ledger`)

Frozen. Additive-only changes under bumped `schema_version`. `writ verify` validates every historical version forever.

- Two-phase records: exactly one `Decision` record per intercepted call (even if execution never starts); one linked `Execution` record per dispatched call (`decision_index` links back). Records are never mutated.
- `record_hash` = SHA-256 over the canonical payload (all fields except `record_hash`, serde_json struct order). `prev_hash` chains records; genesis = 64 zero hex chars.
- `LedgerStore`: `append / tip / get / len / iter`. `append` must reject a non-sequential index, a wrong `prev_hash`, or a `record_hash` that is not the record's own.
- Stores: `FileLedgerStore` (JSONL, default, everywhere) and `SqliteLedgerStore` (WAL mode, `sqlite` feature); Postgres+S3 in Wave 3. All pass the same generic test suite and `verify_chain`. A SQLite row holds the exact JSONL line, so records and hashes are store-independent.
- Store selection: `writ_ledger::open_store(path)` / `detect_store_kind(path)` — content first (SQLite header), then extension (`.db`, `.sqlite`, `.sqlite3`), failing closed on a mismatch or when the `sqlite` feature is off. `verify(path)`, `sessions(path)` and `find_by_call_id(path)` use it; `verify_sqlite(path)` checks a database too damaged to open as a store.
- No content capture beyond call args by default; zero product telemetry.

## Contract 4 — `SandboxBackend` (`writ_core::sandbox`)

`name() / available() / prepare(SandboxSpec) -> SandboxId / exec(id, ExecRequest) -> ExecOutput / collect(id) -> Artifacts / teardown(id)`.
Backends: `local-os` (default), `docker`, `microsandbox`, `firecracker`/`e2b`, `k8s`. Optional and detected, never required (spec §12).

## Contract 5 — `Approver` (`writ_core::approver`)

`request(&ToolCall, &AskView) -> ApprovalOutcome`. Implementations: TUI (workstation), out-of-band (CI), RBAC (cluster). Timeout → `Deny` (fail closed). `ApprovalDecision = AllowOnce | AlwaysAllowRule | Deny | EditArgs(Value)`. `FailClosedApprover` ships in core for headless use.

## Contract 6 — Hook gateway (`writ check`)

The one integration primitive for agents that expose a pre-tool hook (SDK
adapters, Claude Code hooks, any future agent). No daemon: writ runs either
once per call or as a long-lived child process of the agent. Protocol
version `v: 1`; changes are additive only.

**Transport.**
- One-shot: `writ check [--format writ|claude-code]` reads one request from
  stdin, writes one response to stdout, exits. Exit code: `0` = dispatch,
  `2` = do not dispatch, `1` = writ error (callers treat 1 as do-not-dispatch).
- Long-lived: `writ check --stdio` reads newline-delimited JSON requests and
  writes one newline-delimited JSON response per request, in order, until
  stdin closes. stdout carries only protocol lines; diagnostics go to stderr.
- `--policy`/`--ledger` resolve as for every command. Many `writ check`
  processes may share one ledger concurrently (parallel tool calls); the
  ledger store serializes appends across processes.

**Requests** (`--format writ`, the adapter format):

```json
{"v":1,"id":"r1","op":"decide","call":{
  "call_id":"toolu_01","session_id":"thread-42","tool":"bash",
  "args":{"command":"ls"},
  "caller":{"agent":"langgraph","agent_version":"0.3","user":"alice"},
  "server":null,"trust":null}}
{"v":1,"id":"r2","op":"resolve","ref":"<ref from decide>","approved":true,"approver":"human:alice"}
{"v":1,"id":"r3","op":"complete","ref":"<ref>","ok":true,"exit":0,"output":"<tool result as text, optional>"}
```

- `call` fields mirror `ToolCall`; writ sets `mode = SdkHook` and
  `captured_at`. `call_id` is optional (writ generates one); `caller` defaults
  to `{"agent":"unknown"}`; `server`/`trust` use `ToolCall`'s JSON shapes.
  Adapters must never put credentials in `args`.
- `resolve` is only valid after a `decide` that returned `approval:"required"`.
- `complete` records the execution. `output` is hashed into the execution
  record and never stored; for a `redact` verdict writ returns the redacted
  text, which is what the adapter must hand back to the model.

**Responses:**

```json
{"v":1,"id":"r1","decision":"allow","dispatch":true,"rule_id":"read-only","ref":"..."}
{"v":1,"id":"r1","decision":"deny","dispatch":false,"rule_id":"no-rm","reason":"...","location":"writ.yaml:12","ref":"..."}
{"v":1,"id":"r1","decision":"ask","dispatch":false,"approval":"required","rule_id":"prod","reason":"<diff>","irreversible":true,"timeout_ms":60000,"ref":"..."}
{"v":1,"id":"r1","decision":"redact","dispatch":true,"rule_id":"pii","patterns":["\d{3}-\d{2}-\d{4}"],"ref":"..."}
{"v":1,"id":"r3","recorded":true,"output":"<redacted output, only for redact>"}
{"v":1,"id":"r1","error":{"code":"bad_request","message":"..."}}
```

- `ref` is opaque; pass it back unchanged to `resolve`/`complete`.
- `ask` handling is `--ask deny|defer` (default `deny`, fail-closed, since
  `writ check` has no terminal). With `defer`, `ask` comes back with
  `approval:"required"`; the adapter obtains a human decision through the
  agent's own UI and sends `resolve`, whose response is a final decision
  (`dispatch` true or false). An unresolved deferred ask never dispatches.
- Any `error`, malformed line, timeout, or missing writ binary is
  fail-closed in every adapter: the tool does not run.
- Ledger semantics are unchanged (Contract 3): exactly one Decision record
  per intercepted call, one Execution record per `complete`.

**`--format claude-code`.** stdin is a Claude Code hook payload
(`PreToolUse` → decide, `PostToolUse` → complete, keyed by `tool_use_id`
within `session_id`); stdout is Claude Code's hook JSON. A writ `ask` maps to
Claude Code's own permission prompt (`permissionDecision: "ask"`), `deny` to
`"deny"` with the rule's reason, `allow`/`redact` to `"allow"`.
`writ integrate claude-code` writes the hook entries into
`.claude/settings.json` (project) without disturbing existing settings.

## Pipeline (`writ_core::pipeline`)

`handle_call(call, policy, ledger, approver) -> DecisionOutcome` — evaluate → resolve ask via approver → write decision record → return. Dispatch (sandbox exec / MCP forwarding) happens in the caller AFTER `handle_call` returns and only when `outcome.should_dispatch()`. Execution completion → `record_execution(ledger, &decision_record, backend, exit, output)`.

## CLI surface (`writ-cli`)

`writ run [--yolo] [--policy PATH] [--net open|none] [--allow-write PATH]… [--unconfined] [--no-hooks] -- <agent cmd>` · `writ check [--stdio] [--format writ|claude-code] [--ask deny|defer]` · `writ integrate <claude-code>` · `writ proxy --mcp` · `writ log` · `writ show <call-id>` · `writ verify [--ledger PATH]` · `writ replay <run-id>` · `writ policy test` · `writ policy add <pack>` · `writ doctor` · `writ report`. Ledger default path: `.writ/ledger.jsonl` (displayed as `ledger.db` once SQLite is enabled).
