"use strict";
// Locates the prebuilt `writ` binary shipped in the platform package that npm
// installed for this machine (an optionalDependency filtered by os/cpu).

const { existsSync } = require("node:fs");
const { dirname, join } = require("node:path");

const PLATFORMS = {
  "darwin-arm64": "@writ-agent/cli-darwin-arm64",
  "darwin-x64": "@writ-agent/cli-darwin-x64",
  "linux-arm64": "@writ-agent/cli-linux-arm64",
  "linux-x64": "@writ-agent/cli-linux-x64",
  "win32-x64": "@writ-agent/cli-win32-x64",
};

function platformPackage(platform = process.platform, arch = process.arch) {
  return PLATFORMS[`${platform}-${arch}`];
}

/** Absolute path of the bundled writ binary. Throws if none fits this machine. */
function binaryPath() {
  const key = `${process.platform}-${process.arch}`;
  const pkg = platformPackage();
  if (pkg === undefined) {
    throw new Error(
      `writ has no prebuilt binary for ${key}; supported: ${Object.keys(PLATFORMS).join(", ")}. ` +
        "Build from source: cargo install --git https://github.com/writ-agent/writ writ-cli",
    );
  }
  let dir;
  try {
    dir = dirname(require.resolve(`${pkg}/package.json`));
  } catch {
    throw new Error(
      `${pkg} is not installed. It is an optional dependency of @writ-agent/cli; ` +
        "reinstall without --omit=optional / --no-optional.",
    );
  }
  const bin = join(dir, "bin", process.platform === "win32" ? "writ.exe" : "writ");
  if (!existsSync(bin)) throw new Error(`${pkg} is installed but has no binary at ${bin}`);
  return bin;
}

module.exports = { binaryPath, platformPackage, PLATFORMS };
