# DECISIONS.md — Architecture Decision Records (append-only)

## ADR-001: Interception, not orchestration
Writ wraps agents; it does not own the agent loop. Adoption friction is the
primary architectural constraint (spec §5). Consequence: three interception
modes (MCP proxy, process wrap, SDK hooks), never a runtime users must adopt.

## ADR-002: Ledger schema v1 is frozen
`writ verify` must verify every historical ledger version forever (plan §2 Q3).
Changes are additive-only under `schema_version`. Golden fixtures committed.

## ADR-003: Two-phase ledger records
One `Decision` record at verdict time (crash-safe evidence of the decision),
one linked `Execution` record at completion (`decision_index`). Records are
never mutated; mutation would void tamper-evidence.

## ADR-004: Pluggable policy engines behind one IR
Native DSL (default), Rego, Cedar — all compile to `writ_core::Verdict` and
must agree on the shared fixture corpus. Enterprise teams keep their language;
developers get ergonomics (spec §7).

## ADR-005: Default ledger store is JSONL; SQLite is a feature
`FileLedgerStore` (append-only JSONL) works on every platform with zero native
dependencies. `SqliteLedgerStore` (WAL mode, spec §9) ships behind the
`sqlite` cargo feature and becomes the default workstation store once
packaging includes a C toolchain. Both stores pass the same `verify_chain`
suite — the store is swappable, the evidence format is not.

## ADR-006: Windows builds use the GNU toolchain in this environment
The reference build environment lacks MSVC Build Tools; the repo pins nothing —
`rust-toolchain.toml` selects `stable` only. Release binaries are built in CI
on native runners per platform (plan §3.8).
