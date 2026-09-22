// Mark dist/cjs as CommonJS and dist/esm as ESM so Node and TypeScript
// interpret each half of the dual build correctly.
import { writeFileSync } from "node:fs";

writeFileSync(new URL("../dist/cjs/package.json", import.meta.url), JSON.stringify({ type: "commonjs" }) + "\n");
writeFileSync(new URL("../dist/esm/package.json", import.meta.url), JSON.stringify({ type: "module" }) + "\n");

// Per-format self location for the bundled-binary lookup (see src/self.ts).
const esmSelf = "export const selfUrl = import.meta.url;\n";
const cjsSelf = [
  '"use strict";',
  'Object.defineProperty(exports, "__esModule", { value: true });',
  'exports.selfUrl = require("node:url").pathToFileURL(__filename).href;',
  "",
].join("\n");
writeFileSync(new URL("../dist/esm/self.js", import.meta.url), esmSelf);
writeFileSync(new URL("../dist/cjs/self.js", import.meta.url), cjsSelf);
