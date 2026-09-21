# THREAT_MODEL.md

Published from the first commit, per the Writ v2.0 specification §15.
A written threat model with honest exclusions is a stronger trust signal than
any feature list.

## Controls and residual risk

| Threat | Control | Residual risk |
|---|---|---|
| Agent runs a destructive command (hallucinated or injected) | Policy deny on matched patterns; kernel-level filesystem and exec restriction (mode B) | Novel command forms not matched by rules; mitigated by fail-closed default (`default: ask`) |
| Data exfiltration to an attacker-controlled host | Egress allow-list at the process boundary; credential injection at dispatch | Exfiltration through an allow-listed host (e.g. a permitted API with a write endpoint) |
| Malicious or compromised MCP server | Per-server identity, scoped credentials, scanner verdict as a policy input (`server.trust`) | A trusted server that turns malicious between scans |
| Irreversible action taken without authority | `ask` verdict with a human gate; `irreversible` marking | Approval fatigue — the human clicks allow. Mitigated by keeping `ask` rare and diffs precise |
| Audit record altered after the fact | Hash chain; `writ verify`; optional signed receipts (Sigstore keyless) | Tamper-**evident** only until an external anchor (transparency log / append-only export) is configured |
| Agent bypasses Writ entirely | Kernel-enforced process wrap; `writ doctor` coverage report | **In MCP-proxy-only mode this is trivially possible.** Stated loudly in the README and doctor output |

## Explicitly out of scope

- **Prompt injection is not solved.** Writ reduces the blast radius of a
  successful injection; it does not detect or prevent one.
- **Model-layer safety** — harmful content, jailbreaks, output filtering.
- **A malicious operator.** Someone who can edit `writ.yaml` and the ledger is
  inside the trust boundary. Cluster mode narrows this with RBAC and remote
  ledgers; workstation mode does not attempt it.
- **Side-effect reversal.** Writ can prevent and record. It cannot undo.

## Claim discipline (project policy)

No unearned assurance labels, no certification claims (SOC 2 applies to
organisations and hosted services, not binaries), no unbenchmarked performance
numbers. Latency figures live only in reproducible benchmarks in this
repository. Enforced by the `claim-discipline` CI job.
