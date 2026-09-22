import * as assert from "node:assert/strict";
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";
import { pathToFileURL } from "node:url";

import { bundledWrit, locateWrit } from "../src/locate.js";

function fakeInstall(binaryExists: boolean): { root: string; from: string; bin: string } {
  const root = mkdtempSync(join(tmpdir(), "writ-bundled-"));
  const cli = join(root, "node_modules", "@writ-agent", "cli");
  mkdirSync(cli, { recursive: true });
  const bin = join(root, process.platform === "win32" ? "writ.exe" : "writ");
  if (binaryExists) writeFileSync(bin, "");
  writeFileSync(join(cli, "package.json"), JSON.stringify({ name: "@writ-agent/cli", main: "index.js" }));
  writeFileSync(join(cli, "index.js"), `exports.binaryPath = () => ${JSON.stringify(bin)};\n`);
  return { root, from: pathToFileURL(join(root, "sdk.js")).href, bin };
}

test("bundledWrit finds the binary from an installed @writ-agent/cli", () => {
  const f = fakeInstall(true);
  try {
    assert.equal(bundledWrit(f.from), f.bin);
  } finally {
    rmSync(f.root, { recursive: true, force: true });
  }
});

test("bundledWrit is undefined when the binary is missing or cli is absent", () => {
  const f = fakeInstall(false);
  const empty = mkdtempSync(join(tmpdir(), "writ-nocli-"));
  try {
    assert.equal(bundledWrit(f.from), undefined);
    assert.equal(bundledWrit(pathToFileURL(join(empty, "sdk.js")).href), undefined);
    assert.equal(bundledWrit(undefined), undefined);
  } finally {
    rmSync(f.root, { recursive: true, force: true });
    rmSync(empty, { recursive: true, force: true });
  }
});

test("WRIT_BIN still wins over any bundled binary", () => {
  const f = fakeInstall(true);
  try {
    const launch = locateWrit(undefined, { WRIT_BIN: f.bin, PATH: "" });
    assert.equal(launch.command, f.bin);
  } finally {
    rmSync(f.root, { recursive: true, force: true });
  }
});
