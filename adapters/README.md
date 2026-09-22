# Agent integrations

All integrations speak writ's hook-gateway protocol
([`docs/INTERFACES.md`, Contract 6](../docs/INTERFACES.md)) to a `writ check`
child process, so every one of them records the same ledger evidence and
fails closed the same way: no answer from writ, no tool call.

| Package | Frameworks |
|---|---|
| [`python/`](python/) — `writ-agent` | LangGraph, OpenAI Agents SDK, Claude Agent SDK, any callable (`@writ_tool`) |
| [`typescript/`](typescript/) — `@writ-agent/sdk` | Claude Agent SDK, any function or AI-SDK-style tool (`guard`, `guardTools`) |

Claude Code needs no package: `writ integrate claude-code` wires
`writ check --format claude-code` into its hooks, and `writ run -- claude`
does the same for one confined session.

Claude Code tool names map onto writ's policy vocabulary identically in the
Rust gateway and both packages: `Bash`/`PowerShell` → `bash` (`command`),
`Read`/`Glob`/`Grep`/`LS` → `fs.read` (`path`), `Write`/`Edit`/`MultiEdit`/
`NotebookEdit` → `fs.write` (`path`), `WebFetch` → `http` (`url.host`),
`WebSearch` → `web.search` (`query`), `mcp__<server>__<tool>` → `<tool>` on
`server`.
