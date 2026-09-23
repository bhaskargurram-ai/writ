import * as assert from "node:assert/strict";
import { test } from "node:test";

import { WritClient } from "../src/client.js";
import { WritError } from "../src/errors.js";

test('ask "ui" is accepted and waits longer by default', () => {
  const ui = new WritClient({ ask: "ui" }) as unknown as { timeoutMs: number; askMode: string };
  assert.equal(ui.askMode, "ui");
  assert.equal(ui.timeoutMs, 180_000);
  const explicit = new WritClient({ ask: "ui", timeoutMs: 5_000 }) as unknown as { timeoutMs: number };
  assert.equal(explicit.timeoutMs, 5_000);
  const plain = new WritClient() as unknown as { timeoutMs: number };
  assert.equal(plain.timeoutMs, 30_000);
  assert.throws(() => new WritClient({ ask: "maybe" as never }), WritError);
});
