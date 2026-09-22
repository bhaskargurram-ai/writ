# writ

**Nothing runs without a writ.** Every tool call an AI agent makes is checked
against one policy file (`writ.yaml`: allow / deny / ask / redact) before it
runs, and written to a hash-chained, tamper-evident ledger after.

This package installs the prebuilt `writ` command-line tool:

```bash
pip install writ-cli      # or: npm install -g @writ-agent/cli
writ --version
writ integrate claude-code   # govern every Claude Code tool call
writ run -- claude           # or launch an agent inside a kernel write boundary
writ verify                  # check the ledger's hash chain
```

For Python agent frameworks (LangGraph, OpenAI Agents SDK, Claude Agent SDK)
install [`writ-sdk`](https://pypi.org/project/writ-sdk/), which depends on this
package.

Documentation, policy reference and threat model:
https://github.com/writ-agent/writ
