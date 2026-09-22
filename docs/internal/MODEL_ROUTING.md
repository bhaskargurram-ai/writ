# Model Routing Matrix — all agents on GPT-5.5

User override: **every tier and every agent should use GPT-5.5**.

> Runtime note: the Cline team-dispatch API used in this workspace does not expose a per-agent model-selection field. This file is the routing policy for human/operator configuration. Set the active teammate/session model to GPT-5.5 in the provider/Cline settings before dispatching agents.

| Agent | Role | Tier | Model |
|---|---|---|---|
| A0 | Orchestrator / contracts / ADRs | T1 | GPT-5.5 |
| A1 | Policy DSL parser/evaluator | T1 | GPT-5.5 |
| A2 | Rego/Cedar engine adapters | T2 | GPT-5.5 |
| A3 | Ledger/hash-chain/verify | T1 | GPT-5.5 |
| A4 | MCP proxy protocol | T1 | GPT-5.5 |
| A5 | Linux kernel sandboxing | T1 | GPT-5.5 |
| A6 | macOS kernel sandboxing | T1 | GPT-5.5 |
| A7 | Windows sandboxing | T1 | GPT-5.5 |
| A8 | Container/VM backends + benches | T2 | GPT-5.5 |
| A9 | TUI approval gate | T2 | GPT-5.5 |
| A10 | Replay engine | T2 | GPT-5.5 |
| A11 | OTel emitter, SDK adapters | T2/T3 | GPT-5.5 |
| A12 | CLI verbs, doctor, report | T2/T3 | GPT-5.5 |
| A13 | Enterprise: Helm/SSO/RBAC/SIEM | T2 + T1 review | GPT-5.5 |
| A14 | DevOps/release/SBOM/scorecard | T3 | GPT-5.5 |
| A15 | Docs/packs/examples | T3 | GPT-5.5 |
| A16 | Adversarial review / fuzz triage / test generation | T1/T3 | GPT-5.5 |

## Rules

1. All future teammate sessions should run with GPT-5.5.
2. Do not downgrade by task type unless the user explicitly changes this override.
3. If GPT-5.5 is unavailable in the runtime, stop and report the model-availability problem rather than silently using a fallback.
4. Task prompts remain self-contained (see `BUILD_PLAN.md` §11) so restarting agents with GPT-5.5 loses no context.
