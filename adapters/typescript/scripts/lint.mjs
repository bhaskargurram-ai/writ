// Minimal, dependency-free lint: style and safety checks tsc does not do.
import { readdirSync, readFileSync, statSync } from "node:fs";
import { join, relative } from "node:path";
import { fileURLToPath } from "node:url";

const root = fileURLToPath(new URL("..", import.meta.url));
const TARGETS = ["src", "test", "examples", "scripts", "README.md"];
const BANNED = [
  "tamper-proof",
  "tamperproof",
  "bank grade",
  "bank-grade",
  "military grade",
  "military-grade",
  "unbreakable",
  "zero latency",
  "zero-latency",
  "sub-millisecond",
  "prevents prompt injection",
];

function* files(p) {
  const full = join(root, p);
  if (statSync(full).isDirectory()) {
    for (const entry of readdirSync(full)) yield* files(join(p, entry));
  } else if (/\.(ts|mjs|js|md)$/.test(p)) {
    yield p;
  }
}

const problems = [];
for (const target of TARGETS) {
  for (const file of files(target)) {
    // The lint script itself lists the banned words.
    const self = relative(root, fileURLToPath(import.meta.url)).replace(/\\/g, "/");
    const rel = file.replace(/\\/g, "/");
    const text = readFileSync(join(root, file), "utf8");
    const lines = text.split("\n");
    lines.forEach((line, i) => {
      const at = `${rel}:${i + 1}`;
      if (/[ \t]+\r?$/.test(line)) problems.push(`${at}: trailing whitespace`);
      if (line.includes("\t") && !rel.endsWith(".md")) problems.push(`${at}: tab character`);
      if (rel !== self) {
        const lower = line.toLowerCase();
        for (const word of BANNED) if (lower.includes(word)) problems.push(`${at}: banned phrase "${word}"`);
      }
      if (rel.startsWith("src/")) {
        if (/\bconsole\.(log|debug|info)\b/.test(line)) problems.push(`${at}: console output in library code`);
        if (/(:\s*any\b|\bas any\b|<any>)/.test(line)) problems.push(`${at}: explicit any`);
        const imp = /from\s+"(\.{1,2}\/[^"]+)"/.exec(line);
        if (imp && !imp[1].endsWith(".js")) problems.push(`${at}: relative import must end in .js (${imp[1]})`);
        if (/\bshell:\s*true\b/.test(line)) problems.push(`${at}: shell: true is not allowed`);
      }
    });
    if (!text.endsWith("\n")) problems.push(`${rel}: missing final newline`);
  }
}

if (problems.length > 0) {
  console.error(problems.join("\n"));
  console.error(`\n${problems.length} lint problem(s)`);
  process.exit(1);
}
console.log("lint: ok");
