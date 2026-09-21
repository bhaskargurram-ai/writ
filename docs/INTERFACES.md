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
- `LedgerStore`: `append / tip / get / len / iter`. `append` must reject non-sequential index or wrong `prev_hash`.
- Stores: `FileLedgerStore` (JSONL, default, everywhere) and `SqliteLedgerStore` (WAL mode, `sqlite` feature); Postgres+S3 in Wave 3. All pass `verify_chain`.
- No content capture beyond call args by default; zero product telemetry.

## Contract 4 — `SandboxBackend` (`writ_core::sandbox`)

`name() / available() / prepare(SandboxSpec) -> SandboxId / exec(id, ExecRequest) -> ExecOutput / collect(id) -> Artifacts / teardown(id)`.
Backends: `local-os` (default), `docker`, `microsandbox`, `firecracker`/`e2b`, `k8s`. Optional and detected, never required (spec §12).

## Contract 5 — `Approver` (`writ_core::approver`)

`request(&ToolCall, &AskView) -> ApprovalOutcome`. Implementations: TUI (workstation), out-of-band (CI), RBAC (cluster). Timeout → `Deny` (fail closed). `ApprovalDecision = AllowOnce | AlwaysAllowRule | Deny | EditArgs(Value)`. `FailClosedApprover` ships in core for headless use.

## Pipeline (`writ_core::pipeline`)

`handle_call(call, policy, ledger, approver) -> DecisionOutcome` — evaluate → resolve ask via approver → write decision record → return. Dispatch (sandbox exec / MCP forwarding) happens in the caller AFTER `handle_call` returns and only when `outcome.should_dispatch()`. Execution completion → `record_execution(ledger, &decision_record, backend, exit, output)`.

## CLI surface (`writ-cli`)

`writ run [--yolo] [--policy PATH] -- <agent cmd>` · `writ proxy --mcp` · `writ log` · `writ show <call-id>` · `writ verify [--ledger PATH]` · `writ replay <run-id>` · `writ policy test` · `writ policy add <pack>` · `writ doctor` · `writ report`. Ledger default path: `.writ/ledger.jsonl` (displayed as `ledger.db` once SQLite is enabled).
