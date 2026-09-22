# THREAT_MODEL.md

Published from the first commit, per the Writ v2.0 specification §15.
A written threat model with honest exclusions is a stronger trust signal than
any feature list.

## Controls and residual risk

| Threat | Control | Residual risk |
|---|---|---|
| Agent runs a destructive command (hallucinated or injected) | Policy deny on matched patterns; kernel write and network boundary for commands run through the `local-os` sandbox backend (see below) | Novel command forms not matched by rules, mitigated by the fail-closed default (`default: ask`). The agent process launched by `writ run` is not yet inside the kernel boundary |
| Data exfiltration to an attacker-controlled host | Sandboxed commands get no network at all (hostname allow-lists are refused, not approximated); credential injection at dispatch | Exfiltration through an allow-listed host (e.g. a permitted API with a write endpoint) |
| Malicious or compromised MCP server | Per-server identity, scoped credentials, scanner verdict as a policy input (`server.trust`) | A trusted server that turns malicious between scans |
| Irreversible action taken without authority | `ask` verdict with a human gate; `irreversible` marking | Approval fatigue — the human clicks allow. Mitigated by keeping `ask` rare and diffs precise |
| Audit record altered after the fact | Hash chain; `writ verify`; optional signed receipts (Sigstore keyless) | Tamper-**evident** only until an external anchor (transparency log / append-only export) is configured |
| Agent bypasses Writ entirely | Process wrap and MCP proxy as the only paths to credentials; `writ doctor` coverage report | **In MCP-proxy-only mode this is trivially possible**, and `writ run` supervises the launch but does not confine the agent. Stated loudly in the README and doctor output |

## Kernel boundary (`local-os` sandbox backend)

Applies to commands executed through `LocalOsBackend`. By default
(`EnforcementMode::Required`) a spec the running kernel cannot fully enforce
is refused at `prepare`; degraded mode is an explicit opt-in that reports
each gap as enforced, partial, or not enforced. `writ doctor` probes the
running machine rather than assuming from the OS.

| Platform | Mechanism | Enforced | Residuals |
|---|---|---|---|
| Linux | Landlock (all write rights of the running ABI) + seccomp | Writes only under the workspace, a private temp dir and `/dev/{null,zero,full}`; `socket()` of every family, non-unix `socketpair` and `io_uring` return EPERM; holds as root | Reads and exec are not restricted. Landlock ABI v1–2 cannot cover truncate (refused in Required mode). Workspaces on 9p/drvfs (WSL `/mnt/c`) are refused because Landlock misbehaves there |
| Windows | AppContainer token (low IL, no capabilities) + Job Object | Writes only in the workspace and the container profile; network blocked including loopback (when BFE and mpssvc run); kill-on-close, no breakaway, process limit | Objects that grant ALL APPLICATION PACKAGES write access stay writable. Reads are limited to what ALL APPLICATION PACKAGES can read, so tools installed under the user profile will not start. The workspace ACE persists after teardown |
| macOS | Seatbelt (`sandbox_init`, generated profile) | Designed: writes only under workspace and temp; network denied | **Compiled, not yet observed on a real Mac.** Mach IPC is not restricted; reads are not restricted |

No platform can filter egress by hostname at this layer, so a non-empty
`allowed_hosts` fails closed rather than being silently widened.

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
