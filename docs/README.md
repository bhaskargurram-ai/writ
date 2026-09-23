# writ documentation

| Document | What it covers |
|---|---|
| [policy-reference.md](policy-reference.md) | The `writ.yaml` language: rules, conditions, verdicts, defaults, packs |
| [INTERFACES.md](INTERFACES.md) | The frozen contracts between crates: call context, verdict IR, ledger record schema, sandbox backend, CLI surface |
| [THREAT_MODEL.md](THREAT_MODEL.md) | What writ defends against, what it does not, and where each boundary sits |
| [SECURITY.md](SECURITY.md) | How to report a vulnerability, and the supported versions |
| [DECISIONS.md](DECISIONS.md) | Architecture decision records (ADRs) |
| [BRAND.md](BRAND.md) | Name, voice, and claim discipline |
| [ui.md](ui.md) | `writ ui`: the local console, its security model, and `writ check --ask ui` |
| [receipts.md](receipts.md) | Signed receipts, inclusion proofs and anchoring (Contract 7), with their threat model |
| [ledger-postgres.md](ledger-postgres.md) | The Postgres ledger store: setup, roles, schema, concurrency, TLS |
| [mcp-http.md](mcp-http.md) | The MCP proxy over Streamable HTTP / SSE |
| [integrations/](integrations/) | Codex CLI, Gemini CLI, Cursor and Windsurf: setup, mapping, residual risks |

Internal build planning (how the project is being built, not how to use it)
lives in [internal/](internal/).

For a first run, start with the [README](../README.md); for contributing, see
[CONTRIBUTING.md](../CONTRIBUTING.md).
