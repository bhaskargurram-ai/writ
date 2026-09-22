// End-to-end against the real `writ` binary. Skipped unless WRIT_E2E=1.
// Uses WRIT_BIN (or `writ` on PATH).
import * as assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { writeFileSync } from "node:fs";
import { join } from "node:path";
import { describe, it } from "node:test";

import { WritBlockedError, WritClient, guard, locateWrit } from "../src/index.js";
import { tempDir } from "./helpers.js";

const enabled = process.env.WRIT_E2E === "1";

const POLICY = `version: 1
default: ask

rules:
  - id: allow-ls
    when: tool == "bash" and command matches "^ls"
    verdict: allow

  - id: no-rm
    when: tool == "bash" and command matches "rm -rf"
    verdict: deny
    reason: "Destructive command."

  - id: prod-deploy
    when: tool == "bash" and command matches "^deploy"
    verdict: ask
    reason: "Production deploy."

  - id: mask-email
    when: tool == "db.query"
    verdict: redact
    patterns:
      - "[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\\\\.[A-Za-z]{2,}"
`;

describe("e2e: real writ check --stdio", { skip: enabled ? false : "set WRIT_E2E=1 (and WRIT_BIN) to run" }, () => {
  it("allow / deny / redact / default-ask, then writ verify", async () => {
    const dir = tempDir("writ-e2e-");
    const policy = join(dir, "writ.yaml");
    const ledger = join(dir, "ledger.jsonl");
    writeFileSync(policy, POLICY);

    const client = new WritClient({ policy, ledger, cwd: dir, caller: { agent: "writ-sdk-e2e" } });
    try {
      const bash = guard((input: { command: string }) => `ran: ${input.command}`, { client, tool: "bash" });
      assert.equal(await bash({ command: "ls -la" }), "ran: ls -la");

      await assert.rejects(bash({ command: "rm -rf /" }), (e: unknown) => {
        assert.ok(e instanceof WritBlockedError);
        assert.equal(e.decision.rule_id, "no-rm");
        assert.match(e.decision.location ?? "", /writ\.yaml:\d+/);
        return true;
      });

      const query = guard(() => "alice@example.com,42", { client, tool: "db.query" });
      const redacted = await query();
      assert.equal(typeof redacted, "string");
      assert.doesNotMatch(redacted, /alice@example\.com/);
      assert.match(redacted, /42/);

      // Unmatched -> policy default ask -> `--ask deny` -> blocked.
      await assert.rejects(guard((_input: { path: string }) => "never", { client, tool: "fs.write" })({ path: "x" }), WritBlockedError);
    } finally {
      await client.close();
    }

    // Deferred asks: approve one, reject one; completes use the resolve's ref.
    const deferred = new WritClient({ policy, ledger, cwd: dir, ask: "defer" });
    try {
      const deploy = (approved: boolean) =>
        guard((input: { command: string }) => `deployed: ${input.command}`, {
          client: deferred,
          tool: "bash",
          approver: () => ({ approved, approver: "human:e2e" }),
        });
      assert.equal(await deploy(true)({ command: "deploy web" }), "deployed: deploy web");
      await assert.rejects(deploy(false)({ command: "deploy db" }), WritBlockedError);
    } finally {
      await deferred.close();
    }

    const launch = locateWrit();
    const verify = spawnSync(launch.command, [...launch.args, "--ledger", ledger, "verify"], { cwd: dir, encoding: "utf8" });
    assert.equal(verify.status, 0, `writ verify failed:\n${verify.stdout}\n${verify.stderr}`);
  });
});
