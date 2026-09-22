# Agent SDK Adapters

SDK-hook interception (mode C) scaffolds live here:
- `langgraph/` (Python)
- `agents-sdk/` (OpenAI Agents SDK, Python)
- `claude-agent-sdk/` (TypeScript)

Each adapter must normalize native tool events into the frozen `ToolCall` envelope
from `docs/INTERFACES.md` before policy evaluation. The hook contract is the
same for every SDK:

1. Intercept immediately before tool execution.
2. Build `ToolCall { call_id, session_id, caller, mode: SdkHook, tool, args,
   server, trust, captured_at }` without credentials in `args`.
3. Send the envelope to writ (`handle_call` or the local writ daemon) and wait
   for a verdict.
4. Dispatch only on allow / approved ask / successful redact. Deny, timeout,
   missing writ, invalid response, or adapter exception is fail-closed: do not
   execute the tool.
5. Preserve writ ledger and OTel semantics by recording exactly one decision for
   every intercepted call.

These directories intentionally contain lightweight READMEs only for now: no
runtime dependencies are added until concrete SDK packages are selected.
