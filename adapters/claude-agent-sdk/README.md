# writ adapter: Claude Agent SDK

The Claude Agent SDK integration ships in the TypeScript package
[`@writ-agent/sdk`](../typescript/README.md), as the subpath
`@writ-agent/sdk/claude-agent-sdk`:

```ts
import { WritClient } from "@writ-agent/sdk";
import { createWritIntegration } from "@writ-agent/sdk/claude-agent-sdk";

const { hooks, canUseTool } = createWritIntegration({ client: new WritClient() });
// query({ prompt, options: { hooks, canUseTool } })
```

It registers `PreToolUse` (decide), `PostToolUse` / `PostToolUseFailure`
(complete, redaction via `updatedToolOutput`) and a `canUseTool` callback for
deferred asks, over the `writ check --stdio` gateway
([INTERFACES.md Contract 6](../../docs/INTERFACES.md)). Every failure path
denies the tool (fail closed). See the package README for details.
