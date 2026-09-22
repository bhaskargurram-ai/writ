// Wrap any tool function (or a record of { description, parameters, execute } tools).
import { WritBlockedError, WritClient, guard, guardTools } from "@writ-agent/sdk";

async function main(): Promise<void> {
  const writ = new WritClient({ policy: "writ.yaml", caller: { agent: "my-agent", agent_version: "1.0" } });
  try {
    const runQuery = guard(async (input: { query: string }) => `rows for ${input.query}`, {
      client: writ,
      tool: "postgres.query",
    });
    console.log(await runQuery({ query: "select email from users" })); // redacted if a redact rule matches

    const tools = guardTools(
      {
        weather: {
          description: "Get the weather for a city",
          parameters: { type: "object", properties: { city: { type: "string" } } },
          execute: async ({ city }: { city: string }) => `sunny in ${city}`,
        },
      },
      { client: writ, toolName: () => "http" },
    );
    console.log(await tools.weather.execute({ city: "Oslo" }));
  } catch (err) {
    if (err instanceof WritBlockedError) console.error(err.message); // names rule_id and writ.yaml:LINE
    else throw err;
  } finally {
    await writ.close();
  }
}

void main();
