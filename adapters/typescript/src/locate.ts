import { statSync } from "node:fs";
import { createRequire } from "node:module";
import { delimiter, extname, isAbsolute, join, resolve } from "node:path";

import { WritUnavailableError } from "./errors.js";
import { selfUrl } from "./self.js";

/** A resolved program + leading arguments to spawn. */
export interface Launch {
  command: string;
  args: string[];
}

const SCRIPT_EXTENSIONS = new Set([".js", ".mjs", ".cjs"]);

function isFile(p: string): boolean {
  try {
    return statSync(p).isFile();
  } catch {
    return false;
  }
}

/**
 * Turn an explicit binary path into a launch. A `.js`/`.mjs`/`.cjs` path runs
 * under the current Node executable (useful for test gateways on Windows,
 * where scripts are not directly executable).
 */
export function launchFor(bin: string): Launch {
  const full = isAbsolute(bin) ? bin : resolve(bin);
  if (!isFile(full)) {
    throw new WritUnavailableError(`writ binary not found at '${full}' (fail closed: no tool call will run)`);
  }
  if (SCRIPT_EXTENSIONS.has(extname(full).toLowerCase())) {
    return { command: process.execPath, args: [full] };
  }
  return { command: full, args: [] };
}

/** Search PATH for `writ` (`writ.exe` / `writ.com` on Windows). */
export function findOnPath(
  name = "writ",
  env: NodeJS.ProcessEnv = process.env,
  platform: NodeJS.Platform = process.platform,
): string | undefined {
  const pathVar = env.PATH ?? env.Path ?? env.path ?? "";
  const dirs = pathVar.split(platform === "win32" ? ";" : delimiter).filter((d) => d.length > 0);
  // Batch files (.cmd/.bat) need a shell to run; they are not accepted.
  const names = platform === "win32" ? [`${name}.exe`, `${name}.com`] : [name];
  for (const dir of dirs) {
    const clean = dir.replace(/^"(.*)"$/, "$1");
    for (const n of names) {
      const candidate = join(clean, n);
      if (isFile(candidate)) return candidate;
    }
  }
  return undefined;
}

/**
 * The prebuilt binary from the optional `@writ-agent/cli` dependency, if it
 * is installed next to this SDK and ships a binary for this platform.
 */
export function bundledWrit(from: string | undefined = selfUrl): string | undefined {
  if (from === undefined) return undefined;
  try {
    const req = createRequire(from);
    const cli = req("@writ-agent/cli") as { binaryPath?: () => string };
    const path = typeof cli.binaryPath === "function" ? cli.binaryPath() : undefined;
    return path !== undefined && isFile(path) ? path : undefined;
  } catch {
    return undefined;
  }
}

/**
 * Locate writ: explicit `bin`, then `WRIT_BIN`, then the binary bundled by
 * `@writ-agent/cli`, then PATH. Throws `WritUnavailableError` when nothing is
 * found.
 */
export function locateWrit(bin?: string, env: NodeJS.ProcessEnv = process.env): Launch {
  if (bin !== undefined && bin !== "") return launchFor(bin);
  const fromEnv = env.WRIT_BIN;
  if (fromEnv !== undefined && fromEnv !== "") return launchFor(fromEnv);
  const found = bundledWrit() ?? findOnPath("writ", env);
  if (found === undefined) {
    throw new WritUnavailableError(
      "writ binary not found: install @writ-agent/cli, set WRIT_BIN, or put writ on PATH " +
        "(fail closed: no tool call will run)",
    );
  }
  return { command: found, args: [] };
}
