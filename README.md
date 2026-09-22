# writ.

```
$ npx writ run -- claude
  writ · 4 rules loaded from writ.yaml · ledger: .writ/ledger.jsonl
  ✓ read    src/api/handlers.rs
  ✓ bash    cargo test --lib
  ⚠ bash    rm -rf ./build/../../                          [ask]
            rule: block-destructive-shell (writ.yaml:6)
            → path resolves outside the workspace root
            [a]llow  [d]eny  [e]dit  [!] always allow
  ✗ http    POST https://paste.ee/api                      [denied]
            rule: egress-allowlist (writ.yaml:18)

$ writ log       # 47 calls · 2 denied · 1 approved by you
$ writ verify    # chain intact · 47 records · no gaps
```

**Authorization and provenance for AI agents. One policy file, one tamper-evident ledger, any agent.**
For developers who run agents with real credentials, and for the platform teams who answer for it.

## Install and run

```bash
# from source (prebuilt binaries + brew/npx/curl-sh land with the release pipeline)
cargo install --path crates/writ-cli

writ run -- claude        # wrap your agent, policy enforced
```

## The policy that produced the demo

```yaml
version: 1
default: ask                          # fail-closed. --yolo flips this to allow.
rules:
  - id: block-destructive-shell
    when: tool == "bash" and command matches "rm -rf|mkfs|dd if=|:\(\)\{"
    verdict: deny
    reason: "Destructive system command. Narrow the path and retry."
  - id: protect-production-db
    when: tool startswith "postgres" and query matches "(?i)(DROP|TRUNCATE|ALTER)"
    verdict: ask
    irreversible: true
    timeout: 5m
  - id: egress-allowlist
    when: tool == "http" and not url.host in hosts.allowed
    verdict: deny
  - id: never-read-secrets
    when: path matches "\\.env|id_rsa|\\.pem$|credentials$"
    verdict: deny
    reason: "Secrets are masked from the agent by design."
hosts:
  allowed: [api.github.com, registry.npmjs.org, "*.internal.acme.com"]
```

## How it works

```
your agent (unchanged)  — Claude Code · Codex · LangGraph · your own loop
        │ tool call intercepted
        ▼
┌─ WRIT ─────────────────────────────────────────────┐
│  1. INTERCEPT   mcp proxy │ process wrap │ sdk hook │
│  2. DECIDE      writ.yaml → allow · deny · ask · redact │
│  3. RECORD      hash-chained ledger + OTel span     │
└─────────┼──────────────────────────────────────────┘
          │ approved calls only
          ▼
   sandbox backend (local-os · docker · …)  ·  MCP servers
```

- `writ run -- <agent>` — wrap any agent process
- `writ proxy --mcp --server <name> -- <server cmd>` — govern every MCP tool call
- `writ log` / `writ show <call-id>` — what did my agent actually do
- `writ verify` — check whether the local hash chain was edited (names the exact broken record it can verify)
- `writ policy test` — unit-test your rules against recorded fixtures
- `writ doctor` — honest coverage report: what is governed, what is blind
- `writ report` — shareable single-file HTML run summary

## What it does not do

Writ governs **actions**, not reasoning. It does **not** detect or prevent
prompt injection — it shrinks the blast radius (least privilege, egress
allow-lists, a human gate on irreversible actions). The ledger is
tamper-**evident**: local verification catches broken hashes or chain links, but
anyone with write access can delete or rewrite the whole unanchored ledger; tamper-**proof** requires an external anchor
(transparency-log receipts, on the roadmap). And in MCP-proxy-only mode the
agent's own shell, file writes and direct HTTP are not governed — pair mode A
with process wrap or SDK hooks. Full details:
[docs/THREAT_MODEL.md](docs/THREAT_MODEL.md).

## Compatibility

| Layer | Supported | Status |
|---|---|---|
| Agents | any via `run`/`proxy`; SDK hooks for LangGraph, OpenAI Agents SDK, Claude Agent SDK | hooks: wave 3 |
| Sandbox backends | local-os (default) · docker · microsandbox · e2b/firecracker · k8s | local-os shipped; rest wave 2–3 |
| Policy engines | native DSL (shipped) · Rego · Cedar | Rego/Cedar: wave 3, same verdict IR |
| Transports | MCP stdio · SSE · streamable HTTP | stdio shipped; SSE/HTTP: wave 2 |
| Platforms | macOS (arm64/x86_64) · Linux (glibc/musl) · Windows | build matrix in CI |

## Compared to the neighbours, fairly

- **Agent hooks** (vendor or the Leash/Fence/Cordon cluster): per-agent, per-machine, no portable policy, no verifiable record. Writ's policy file moves with the repo; the ledger survives the session.
- **Sandboxes** (E2B, microsandbox, Dagger): excellent isolation, but no per-call decision point, no "ask me before merging to main", no evidence of what was attempted. Writ drives them; it doesn't replace them.
- **MCP gateways** (Docker MCP Gateway, ContextForge): govern MCP traffic only — blind to the shell command the agent runs itself.
- **Observability** (Langfuse, Phoenix, LangSmith): they watch; they cannot stop anything, and their logs are mutable application logs, not evidence.

## Status

Wave 1 (core spine) is implemented and tested: native policy engine (four
verdicts, hot-reload, fixture tests), hash-chained ledger with verify, MCP
stdio proxy with structured refusals and credential injection, local-os
sandbox, approval-gate TUI, and the full CLI (`run · proxy · log · show ·
verify · policy test/add · doctor · report`). See
[WRIT_MASTER_BUILD_PLAN.md](WRIT_MASTER_BUILD_PLAN.md) for the full build
program and honest wave status.

## Links

- [Threat model](docs/THREAT_MODEL.md) · [Security policy](docs/SECURITY.md) · [Policy reference](docs/policy-reference.md)
- [Interfaces (frozen contracts)](docs/INTERFACES.md) · [Decisions (ADRs)](docs/DECISIONS.md)
- Contributing: DCO sign-off, no CLA · License: [Apache-2.0](LICENSE), permanently

**Nothing runs without a writ.**
