// Build the per-platform npm packages (@writ-agent/cli-<os>-<cpu>) from the
// release binaries. Each package declares os/cpu, so npm installs only the one
// that matches the machine, and carries bin/writ(.exe).
//
// usage: node make_platform_packages.mjs <binaries-dir> <out-dir> <version>
// <binaries-dir> holds writ-<rust-target>[.exe] as produced by release.yml.
import { chmodSync, copyFileSync, existsSync, mkdirSync, rmSync, writeFileSync } from "node:fs";
import { join } from "node:path";

const TARGETS = [
  { rust: "aarch64-apple-darwin", os: "darwin", cpu: "arm64" },
  { rust: "x86_64-apple-darwin", os: "darwin", cpu: "x64" },
  { rust: "aarch64-unknown-linux-musl", os: "linux", cpu: "arm64" },
  { rust: "x86_64-unknown-linux-musl", os: "linux", cpu: "x64" },
  { rust: "x86_64-pc-windows-msvc", os: "win32", cpu: "x64" },
];

const [binDir, outDir, version] = process.argv.slice(2);
if (!binDir || !outDir || !/^\d+\.\d+\.\d+(-[\w.]+)?$/.test(version ?? "")) {
  console.error("usage: node make_platform_packages.mjs <binaries-dir> <out-dir> <version>");
  process.exit(2);
}

rmSync(outDir, { recursive: true, force: true });
for (const t of TARGETS) {
  const exe = t.os === "win32" ? ".exe" : "";
  const src = join(binDir, `writ-${t.rust}${exe}`);
  if (!existsSync(src)) {
    console.error(`missing binary: ${src}`);
    process.exit(1);
  }
  const name = `@writ-agent/cli-${t.os}-${t.cpu}`;
  const dir = join(outDir, `cli-${t.os}-${t.cpu}`);
  mkdirSync(join(dir, "bin"), { recursive: true });
  const dest = join(dir, "bin", `writ${exe}`);
  copyFileSync(src, dest);
  if (!exe) chmodSync(dest, 0o755);
  writeFileSync(
    join(dir, "package.json"),
    JSON.stringify(
      {
        name,
        version,
        description: `The prebuilt writ binary for ${t.os}-${t.cpu} (${t.rust}). Install @writ-agent/cli instead.`,
        license: "Apache-2.0",
        author: "Bhaskar Gurram <gurrambhaskar.ai@gmail.com>",
        homepage: "https://github.com/writ-agent/writ",
        repository: { type: "git", url: "git+https://github.com/writ-agent/writ.git" },
        os: [t.os],
        cpu: [t.cpu],
        files: ["bin/"],
        publishConfig: { access: "public" },
        preferUnplugged: true,
      },
      null,
      2,
    ) + "\n",
  );
  writeFileSync(
    join(dir, "README.md"),
    `# ${name}\n\nThe prebuilt \`writ\` binary for ${t.os}-${t.cpu}. Do not install this directly: ` +
      "install [`@writ-agent/cli`](https://www.npmjs.com/package/@writ-agent/cli), which picks the right platform package.\n",
  );
  console.log(`${name}@${version}  <- ${src}`);
}
