# writ

**Nothing runs without a writ.** Every tool call an AI agent makes is checked
against one policy file (`writ.yaml`: allow / deny / ask / redact) before it
runs, and written to a hash-chained, tamper-evident ledger after.

This package installs the prebuilt `writ` command-line tool (npm fetches only the binary for your platform):

```bash
npm install -g @writ-agent/cli   # or: pip install writ-cli
writ --version
writ integrate claude-code   # govern every Claude Code tool call
writ run -- claude           # or launch an agent inside a kernel write boundary
writ verify                  # check the ledger's hash chain
```

For Node agents and the Claude Agent SDK, install
[`@writ-agent/sdk`](https://www.npmjs.com/package/@writ-agent/sdk), which depends on this
package.

Documentation, policy reference and threat model:
https://github.com/writ-agent/writ
