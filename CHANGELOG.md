# Changelog

All notable changes to writ are recorded here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and writ uses
[Semantic Versioning](https://semver.org/) from its first tagged release.
The ledger record schema is versioned separately (`schema_version`, see
[docs/INTERFACES.md](docs/INTERFACES.md)) and only ever changes additively.

## [Unreleased]

### Added

- **`writ ui`**, a local web console: connect an agent (snippets with your
  paths, live "connected" signal), a live decision feed with full record
  detail and chain verification, an Approvals screen for `writ check --ask ui`,
  a policy editor with a "try a tool call" tester, and a sandbox screen that
  runs commands in the kernel boundary and shows escape attempts failing.
  Loopback-only, token-gated, strict CSP, no external requests.
- **More agents**: `writ check --format codex|gemini|cursor|windsurf`,
  `writ integrate` for each, and `writ run` hook injection for Codex, Gemini
  CLI and Cursor's `agent`. Every writ-side error returns that agent's
  blocking answer.
- **Signed receipts** (`writ receipt keygen|create|verify|prove|anchor`):
  Ed25519ph over a ledger checkpoint with an RFC 6962 Merkle root, per-call
  inclusion proofs, and anchoring in the Sigstore Rekor public log (verified
  offline against a pinned log key) or an append-only file.
- **Postgres ledger store** (`postgres` feature): many hosts append one
  chain under a row lock; append-only triggers; TLS via rustls with libpq
  `sslmode` semantics.
- **MCP proxy over Streamable HTTP / SSE** (`writ proxy --transport http`).
- **Browser playground** on the website: the real engine as WebAssembly.
- `--ask ui` in the Python and TypeScript SDKs.

### Security

- A Postgres ledger URL is never printed with its password, and never written
  into agent hook configuration or a `writ run` hook command line.

## [0.1.1] — 2026-09-23

### Added

- **Prebuilt, signed `writ` binaries** for Linux x64/arm64 (static musl),
  macOS arm64/x86_64 and Windows x64 on every release, with checksums,
  Sigstore bundles, SBOMs and build provenance.
- **`writ-cli` on PyPI** (platform wheels carrying the binary) and
  **`@writ-agent/cli` on npm** (per-platform packages; npm installs only the
  one for your machine). `writ-sdk` and `@writ-agent/sdk` depend on them, so
  installing an SDK installs the binary — no repository or Rust toolchain.
- One tag-driven release workflow publishes GitHub Releases, PyPI and npm
  (with npm provenance); versions are kept in lockstep by
  `scripts/check_versions.py`.

## [0.1.0] — 2026-09-22

First published SDKs (`writ-sdk`, `@writ-agent/sdk`); they required building
the `writ` binary from source. Everything listed below under "Added",
"Changed" and "Fixed" shipped on `main` by this point.

### Added

- **Hook gateway `writ check`** (INTERFACES Contract 6): one-shot or
  `--stdio`, decide / resolve / complete, deferred asks, redaction of tool
  output, fail-closed on every error. Refs are checked against the ledger and
  work across processes.
- **Claude Code integration**: `writ check --format claude-code` hooks and
  `writ integrate claude-code`. A writ `ask` becomes Claude Code's own
  permission prompt; every writ-side failure blocks the call (exit 2).
- **`writ-sdk` Python package** ([PyPI](https://pypi.org/project/writ-sdk/), 0.1.0): LangGraph, OpenAI Agents SDK and Claude
  Agent SDK integrations plus `@writ_tool` for any callable.
- **`@writ-agent/sdk` TypeScript package** ([npm](https://www.npmjs.com/package/@writ-agent/sdk), 0.1.0): Claude Agent SDK hooks,
  `guard()` and `guardTools()`.
- The JSONL ledger is safe for many concurrent writer processes (lock file,
  tip re-read from disk).
- Brand: logo, icon, README hero and social preview.
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

- The MCP proxy passed output through unmasked when a redact pattern was
  invalid; it now withholds the output.
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
