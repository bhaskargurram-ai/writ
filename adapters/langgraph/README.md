# writ adapter: LangGraph

Lightweight scaffold for a Python LangGraph SDK-hook adapter. No LangGraph
dependency is vendored here yet.

## Hook contract

Install the adapter as the last pre-tool hook around every LangGraph tool node
or tool executor, immediately before the native callable is invoked.

For each pending tool call, normalize the SDK event to the frozen writ
`ToolCall` envelope:

- `call_id`: LangGraph tool-call id if present, otherwise a generated stable id.
- `session_id`: graph/thread/run id that groups the agent session.
- `caller`: agent name/version and user/non-human identity available from graph
  config.
- `mode`: `SdkHook`.
- `tool`: LangGraph tool name.
- `args`: JSON object passed to the tool, with secrets/credentials omitted.
- `server`: normally `None` for in-process tools; set when proxying to MCP.
- `trust`: upstream trust verdict when available.
- `captured_at`: adapter capture timestamp.

Send that `ToolCall` to writ (`handle_call` in-process or a local writ daemon)
and execute only when the returned outcome dispatches: allow, approved ask, or
redact (execute, then redact tool results before they re-enter model context). Deny, ask timeout,
missing writ, malformed response, policy/ledger error, or adapter exception is
fail-closed: raise a tool error and do not call the LangGraph tool.

The adapter must produce exactly one writ decision for every intercepted
LangGraph tool call and must not leak credentials into `ToolCall.args`.
