# writ adapter: Claude Agent SDK

Lightweight scaffold for a TypeScript Claude Agent SDK hook adapter. No SDK
dependency is vendored here yet.

## Hook contract

Register a pre-tool-use callback/middleware that runs immediately before the
Claude Agent SDK dispatches a tool.

For each pending tool use, normalize the SDK event to the frozen writ `ToolCall`
envelope:

- `call_id`: Claude tool-use id if present, otherwise a generated stable id.
- `session_id`: conversation/session/run id.
- `caller`: agent name/version plus user or non-human principal when available.
- `mode`: `SdkHook`.
- `tool`: Claude tool name.
- `args`: JSON input for the tool, with credentials and injected secrets omitted.
- `server`: `None` for local tools; populate for MCP or named remote servers.
- `trust`: upstream server/tool trust metadata when available.
- `captured_at`: adapter capture timestamp.

Send the `ToolCall` to writ (`handle_call` via a native binding or a local writ
daemon) and dispatch only when the outcome dispatches: allow, approved ask, or
redact (execute, then redact tool results before they re-enter model context). Deny, approval
timeout, unavailable writ, malformed response, policy/ledger failure, or adapter
exception is fail-closed: surface a blocked-tool result and do not invoke the
Claude tool.

The adapter must produce exactly one writ decision for every intercepted Claude
Agent SDK tool use and must not include credentials in `ToolCall.args`.
