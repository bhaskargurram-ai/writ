# Model Routing Matrix — which model for which agent

The build plan (§5) tiers tasks T1/T2/T3. This file maps those tiers onto
concrete model families so no premium token is ever spent on cheap work.
Configure per-session models in your Cline provider settings; this table is
the assignment sheet.

| Agent | Role | Tier | Recommended models (any one) | Avoid |
|---|---|---|---|---|
| A0 | Orchestrator / contracts / ADRs | T1 | Claude Opus 4.x, GPT-5-pro, Kimi K3-thinking, GLM-4.6 | small models — contract errors compound |
| A1 | Policy DSL parser/evaluator | T1 | Claude Opus/Sonnet 4.x, Kimi K3, GLM-4.6 | economy tiers — security-path parser |
| A2 | Rego/Cedar engine adapters | T2 | Claude Sonnet 4.x, Kimi K2, GLM-4.5 | — |
| A3 | Ledger/hash-chain/verify | T1 | Claude Opus 4.x, Kimi K3-thinking, GLM-4.6 | — |
| A4 | MCP proxy protocol | T1 | Claude Sonnet/Opus 4.x, Kimi K3, GLM-4.6 | — |
| A5–A7 | Kernel sandboxing (Linux/macOS/Windows) | T1 | Claude Opus 4.x, GPT-5-pro, Kimi K3-thinking | anything small — syscall security |
| A8 | Container/VM backends + benches | T2 | Claude Sonnet 4.x, Kimi K2, GLM-4.5, Qwen3-Coder | — |
| A9 | TUI approval gate | T2 | Claude Sonnet 4.x, Kimi K2, GLM-4.5 | — |
| A10 | Replay engine | T2 | Claude Sonnet 4.x, Kimi K2, GLM-4.5 | — |
| A11 | OTel emitter, SDK adapters | T2/T3 | Kimi K2, GLM-4.5-Air, Claude Haiku 4.x | — |
| A12 | CLI verbs, doctor, report | T2/T3 | Kimi K2, GLM-4.5-Air, Qwen3-Coder | — |
| A13 | Enterprise (Helm/SSO/RBAC/SIEM) | T2 + T1 design review | Sonnet for impl; Opus/K3-thinking for RBAC/SSO design review | — |
| A14 | DevOps/release/SBOM/scorecard | T3 | GLM-4.5-Air, Kimi K2, Claude Haiku 4.x | frontier — pure plumbing |
| A15 | Docs/packs/examples | T3 | GLM-4.5-Air, Kimi K2, Claude Haiku 4.x | frontier — writes to a spec |
| A16 | Adversarial review / fuzz triage | T1 (review) | Claude Opus 4.x, Kimi K3-thinking, GLM-4.6 | — |
| A16 | Test/fixture generation | T3 | GLM-4.5-Air, Kimi K2 | — |

## Rules
1. Two failed attempts at one tier → escalate one tier up.
2. T1 completion → follow-up polish may be downgraded a tier.
3. Never run T1 models on fixture/docs/CI-plumbing volume work.
4. Every task prompt is self-contained (see plan §11 template) so switching
   models between sessions loses no context.
