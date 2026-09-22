# Changelog

All notable changes to writ are recorded here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and writ uses
[Semantic Versioning](https://semver.org/) from its first tagged release.
The ledger record schema is versioned separately (`schema_version`, see
[docs/INTERFACES.md](docs/INTERFACES.md)) and only ever changes additively.

## [Unreleased]

Nothing has been tagged yet; everything below is on `main`.

### Added

- **Policy engines.** Rego (`writ-policy-rego`, via regorus) and Cedar
  (`writ-policy-cedar`, via cedar-policy) backends behind the same
  `PolicyEngine` trait, both verdict-identical to the native engine on the
  shared fixture corpus and fail-closed on any evaluation error.
- **SQLite ledger store** (`sqlite` feature): WAL, `synchronous=FULL`,
  append-only triggers, and concurrent writers that cannot fork the chain.
  Rows hold the exact JSONL record, so both stores produce the same hashes.
  `open_store` picks the store by content, then extension, failing closed.
- **Kernel sandbox for `local-os`**: Landlock + seccomp on Linux,
  AppContainer + Job Object on Windows, Seatbelt on macOS. Writes are
  confined to the workspace and a private temp dir, and network is denied.
  A spec the running kernel cannot fully enforce is refused unless
  best-effort mode is chosen explicitly. Tests assert the exact errno or
  Win32 error from inside the sandbox. `writ run` does not use it yet.
- **Docker sandbox backend** (`writ-sandbox-docker`): network `none` by
  default, workspace bind mount, per-exec deadlines, idempotent teardown.
- **Benchmarks** (`crates/writ-bench`, criterion) for policy evaluation,
  ledger append/verify and the MCP codec.
- **Fuzzing** (`fuzz/`, cargo-fuzz) of the policy compiler, ledger record
  decoding and the MCP codec, run weekly in CI.
- **CI**: cargo-deny (advisories, licenses, sources), claim-discipline lint
  (`scripts/claim_lint.py`), bench compilation, policy-pack validation, and
  `--all-features` test runs on Linux, macOS and Windows.
- Landing site (`site/`), code of conduct, CODEOWNERS, `.editorconfig`,
  `.gitattributes`, and a documentation index (`docs/README.md`).

### Changed

- The repository moved to [github.com/writ-agent/writ](https://github.com/writ-agent/writ).
- Ledger stores reject an append whose `record_hash` is not the record's own.
- `writ-policy` re-exports `default_verdict`, `verdict_for` and `eval_expr`
  so every engine builds verdicts from one implementation.
- Windows development uses the MSVC toolchain (ADR-009); the GNU-toolchain
  workarounds are gone.
- Internal build planning moved to `docs/internal/` (`BUILD_PLAN.md`,
  `MODEL_ROUTING.md`); `writ doctor` and the threat model now state that the
  kernel boundary does not cover the agent `writ run` launches.

### Fixed

- Parallel tests on macOS shared one temp directory (microsecond clock), so
  replay tests wrote into one ledger.
- Concurrent opens of a SQLite ledger could fail with "database is locked".
- `fuzz_policy_compile` did not compile.

## Foundations (September 2026)

The first build waves, before this changelog existed:

- Frozen contracts in `writ-core`: `ToolCall`, the verdict IR, ledger record
  schema v1, `SandboxBackend`, `Approver`, and the decision pipeline.
- Native `writ.yaml` engine: parser, four verdicts, hot reload, fixture harness.
- Hash-chained JSONL ledger with `verify`, `log` and `show`.
- MCP stdio proxy with structured refusals and credential injection.
- `local-os` sandbox backend, approval gate TUI, and the `writ` CLI:
  `run`, `proxy`, `log`, `show`, `verify`, `replay`, `policy test`,
  `doctor`, `report`.
- OpenTelemetry GenAI spans, trajectory replay, three policy packs, release
  pipeline, threat model and ADRs.
