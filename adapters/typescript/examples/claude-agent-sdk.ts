// Run a Claude Agent SDK session with every tool call checked by writ.
// Requires `writ` on PATH (or WRIT_BIN) and a writ.yaml in the working directory.
import { query } from "@anthropic-ai/claude-agent-sdk";

import { WritClient } from "@writ-agent/sdk";
import { createWritIntegration } from "@writ-agent/sdk/claude-agent-sdk";

async function main(): Promise<void> {
  await using writ = new WritClient({ ask: "defer" });
  const { hooks, canUseTool } = createWritIntegration({
    client: writ,
    // Deferred asks come here; return true only for an explicit human yes.
    approver: async ({ call, decision }) => {
      console.error(`approval needed for ${call.tool}: ${decision.reason ?? ""}`);
      return false;
    },
  });

  for await (const message of query({ prompt: "List the files in this repo", options: { hooks, canUseTool } })) {
    if (message.type === "result") console.log(message);
  }
}

main().catch((err: unknown) => {
  console.error(err);
  process.exitCode = 1;
});
