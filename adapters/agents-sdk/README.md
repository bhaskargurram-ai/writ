# writ adapter: OpenAI Agents SDK

Lightweight scaffold for a Python OpenAI Agents SDK hook adapter. No SDK
dependency is vendored here yet.

## Hook contract

Register as a tool-call guardrail/hook that runs immediately before an OpenAI
Agents SDK tool is invoked.

For each pending tool execution, normalize the SDK event to the frozen writ
`ToolCall` envelope:

- `call_id`: SDK tool-call id from the run item/event, or a generated stable id.
- `session_id`: agent run/session/thread id.
- `caller`: agent name/version plus user or non-human principal when available.
- `mode`: `SdkHook`.
- `tool`: OpenAI Agents SDK tool/function name.
- `args`: JSON arguments for the tool, with credentials and injected secrets
  removed.
- `server`: `None` for in-process tools; populate when the SDK is using a named
  MCP/tool server.
- `trust`: upstream tool/server trust metadata when available.
- `captured_at`: adapter capture timestamp.

Submit the `ToolCall` to writ (`handle_call` in-process or a local writ daemon)
and continue only when the outcome dispatches: allow, approved ask, or
redact (execute, then redact tool results before they re-enter model context). Deny, ask timeout, writ unavailable,
malformed response, policy/ledger failure, or adapter exception is fail-closed:
return/raise a tool-blocked result and do not invoke the SDK tool.

The adapter must write exactly one writ decision for every intercepted OpenAI
Agents SDK tool call and must keep credentials out of `ToolCall.args`.
