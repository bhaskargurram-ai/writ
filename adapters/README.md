# Agent SDK Adapters

SDK-hook interception (mode C) implementations live here:
- `langgraph/` (Python)
- `agents-sdk/` (OpenAI Agents SDK, Python)
- `claude-agent-sdk/` (TypeScript)

Each adapter registers a pre-tool-use callback that asks `writ` for a verdict
and enforces allow/deny/ask/redact. See docs/INTERFACES.md.
