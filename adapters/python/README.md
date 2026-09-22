# writ-agent

Python integrations for [writ](../../README.md). Every tool call an agent makes
is checked against `writ.yaml` (allow / deny / ask / redact) before the tool
runs, and recorded to writ's hash-chained ledger.

The package has no runtime dependencies. It starts `writ check --stdio` as a
long-lived child process and speaks the hook-gateway protocol
([`docs/INTERFACES.md`, Contract 6](../../docs/INTERFACES.md)). Framework
integrations import their framework only when you import them.

## Install

```bash
pip install writ-agent                     # core: WritClient, Writ, @writ_tool
pip install "writ-agent[langgraph]"        # + LangGraph / LangChain
pip install "writ-agent[openai-agents]"    # + OpenAI Agents SDK
pip install "writ-agent[claude-agent-sdk]" # + Claude Agent SDK
```

You also need the `writ` binary. The client looks for it in this order: the
`binary=` argument, the `WRIT_BIN` environment variable, then `writ` on `PATH`.
`policy=` / `ledger=` become `--policy` / `--ledger`; otherwise writ uses
`./writ.yaml` and `./.writ/ledger.jsonl` relative to the child's `cwd`.

## Fail closed

The tool does not run unless writ answers `dispatch: true`. Every failure
raises a `WritError` subclass, and each framework integration turns that into
a refusal the model can read:

| Situation                                        | Result                          |
|--------------------------------------------------|---------------------------------|
| `deny` verdict                                   | `WritDenied` (rule, reason, `writ.yaml:LINE`) |
| `ask`, approver rejects / times out / raises     | `WritApprovalRejected`          |
| `ask` with `--ask deny` (the default)            | `WritApprovalRejected`          |
| writ binary missing or cannot start              | `WritUnavailable`               |
| `writ check` exits while a request is waiting    | `WritGatewayCrashed`            |
| no answer within `timeout` (child is killed)     | `WritTimeout`                   |
| non-JSON line, wrong id, contradictory response  | `WritProtocolError`             |
| `{"error": ...}` response                        | `WritGatewayError`              |
| `complete` fails after the tool ran              | output withheld from the model  |
| `redact` verdict but no redacted output returned | output withheld from the model  |

A request is never retried: a retried `decide` could write a second Decision
record for one call. A crashed gateway is restarted on the next request, up to
`max_restarts` (default 3) per `restart_window` (60 s); past that the client
refuses to start writ again.

## Core API

```python
from writ_agent import Writ, writ_tool

writ = Writ(policy="writ.yaml", session_id="run-42")   # one writ check process

@writ.tool("fs.read")                 # the tool name your policy matches on
def read_file(path: str) -> str:
    return open(path).read()

read_file("README.md")                # decide -> run -> complete; WritDenied if refused
safe = writ.guarded(lambda command: ..., name="bash")
writ.execute("postgres.query", {"query": sql}, lambda: db.run(sql))  # redact -> redacted text
```

The sequence is always: `decide`; for a deferred ask, the `approver` plus
`resolve`; run; `complete`. For a `redact` verdict the value returned is the
redacted **text** writ sends back, not the original object.

`WritClient` / `AsyncWritClient` expose the raw protocol (`decide`, `resolve`,
`complete`) with typed `Decision` / `Completion` results. Both are thread-safe
and share one child process; `close()` (or `with`) closes its stdin and waits
for it to exit, and an `atexit` hook closes anything left open.

### Approvals

With no approver, `Writ` runs `--ask deny`: every `ask` fails closed. Give it
an approver and it runs `--ask defer`; the approver sees the call and the
rule's diff, and its answer goes to writ with `resolve`:

```python
from writ_agent import Approval, Writ

def approve(req):                      # sync or async
    print(req.decision.rule_id, req.diff, req.call.args)
    return Approval(input("run it? [y/N] ") == "y", approver="human:alice")

writ = Writ(approver=approve, approval_timeout=120)   # timeout -> rejected
```

Anything other than `True` / `Approval(True, ...)` rejects, as do exceptions
and timeouts (default: the rule's `timeout_ms`, else 300 s).

## LangGraph / LangChain

Integration point: `ToolNode(wrap_tool_call=..., awrap_tool_call=...)`.
ToolNode hands every tool call to the wrapper with an `execute` callable; the
wrapper asks writ first. A blocked call becomes a `ToolMessage(status="error")`
with writ's reason, so the model sees the refusal and the graph keeps going.
The session id is the graph's `thread_id`.

```python
from langchain_core.tools import tool
from langgraph.prebuilt import tools_condition
from writ_agent import Writ
from writ_agent.langgraph import writ_tool_node

@tool
def read_file(path: str) -> str:
    """Read a file."""
    return open(path).read()

writ = Writ()
graph.add_node("tools", writ_tool_node([read_file], writ))   # instead of ToolNode([...])
graph.add_conditional_edges("model", tools_condition)
app = graph.compile(); app.invoke(inputs, {"configurable": {"thread_id": "t-1"}})
```

`writ_tool_node` accepts every `ToolNode` argument; your own `wrap_tool_call`
runs outside writ's, so writ decides on the request as it will execute. It
can also be passed as `tools=` to `create_react_agent`. For tools invoked
outside a ToolNode, `guard_tool(tool, writ)` wraps a single `BaseTool` (use
one or the other, not both: each records its own decision). For a redact
verdict the `ToolMessage` content is replaced and its `artifact` dropped; a
`Command` result under a redact rule is withheld.

## OpenAI Agents SDK

Integration point: each `FunctionTool`'s `on_invoke_tool`, wrapped. That is
the call `Runner` awaits to execute a tool, so the wrapper can decide before
the tool body runs, record `complete`, and substitute the redacted output.
`RunHooks.on_tool_start` cannot refuse a single call (its return value is
ignored; raising aborts the run), and tool input guardrails see no output, so
they cannot carry a redact verdict.

```python
from agents import Agent, RunConfig, Runner, function_tool
from writ_agent import Writ
from writ_agent.openai_agents import guard_agent

@function_tool
def read_file(path: str) -> str:
    """Read a file."""
    return open(path).read()

writ = Writ()
agent = guard_agent(Agent(name="coder", tools=[read_file]), writ)
result = await Runner.run(agent, "read README.md", run_config=RunConfig(group_id="t-1"))
```

A blocked call returns writ's refusal as the tool result. Session id:
`session_id=` (string or `ctx -> str`), else `RunConfig.group_id`, else a
`session_id` on your run context, else the `Writ`'s. `guard_agent` refuses
agents with tools it cannot gate (hosted tools, `mcp_servers`) unless you pass
`strict=False`; put MCP servers behind `writ proxy --mcp` instead. Handoff
targets are separate agents: guard each one. `guard_function_tool` wraps a
single tool.

## Claude Agent SDK

Integration point: SDK hooks. `PreToolUse` decides (allow / deny with writ's
reason), `PostToolUse` records `complete` and, for a redact verdict, returns
`updatedToolOutput` with the redacted result, `PostToolUseFailure` records the
failure. `can_use_tool` is not used: the CLI only consults it when its own
permission rules would prompt, and it has no post-execution step.

```python
from claude_agent_sdk import ClaudeAgentOptions, ClaudeSDKClient
from writ_agent import Writ
from writ_agent.claude_agent_sdk import writ_hooks

writ = Writ()
options = ClaudeAgentOptions(hooks=writ_hooks(writ))
async with ClaudeSDKClient(options) as client:
    await client.query("list the repo and summarize README.md")
    async for message in client.receive_response():
        print(message)
```

Tool names are mapped exactly as `writ check --format claude-code` maps them
([`toolmap.py`](src/writ_agent/toolmap.py)), so one `writ.yaml` covers Claude
Code and the SDK:

| Claude tool                                   | writ `tool`  | policy field           |
|-----------------------------------------------|--------------|------------------------|
| `Bash`, `PowerShell`                          | `bash`       | `command`              |
| `Read`, `Glob`, `Grep`, `LS`, `NotebookRead`  | `fs.read`    | `path` (from `file_path`) |
| `Write`, `Edit`, `MultiEdit`, `NotebookEdit`  | `fs.write`   | `path` (from `file_path` / `notebook_path`) |
| `WebFetch`                                    | `http`       | `url` (`url.host`)     |
| `WebSearch`                                   | `web.search` | `query`                |
| `mcp__<server>__<tool>`                       | `<tool>`     | `server = <server>`    |
| anything else                                 | unchanged    | unchanged              |

Redacted output keeps the tool's output shape (Claude Code rejects a
mismatched `updatedToolOutput` for built-in tools); if the redacted result
cannot be parsed back into the same shape, every string in the output is
masked. The hooks never raise; any adapter failure answers `deny`.

## Development

```bash
python -m venv .venv && .venv/Scripts/python -m pip install -e ".[dev]"   # bin/ on POSIX
.venv/Scripts/python -m pytest
WRIT_E2E=1 WRIT_BIN=/path/to/writ .venv/Scripts/python -m pytest tests/test_e2e_writ.py
```

Unit tests run against `tests/fake_writ.py`, a small Contract 6 gateway that
picks verdicts by tool name and can misbehave on request (malformed lines,
hangs, crashes, contradictions). Framework tests use each framework's real
runtime with a scripted model: a LangGraph `StateGraph`, the Agents SDK's
`agents.testing.ScriptedModel`, and a scripted stand-in for the Claude Code
CLI behind the SDK's `Transport`. No network, no API keys. The e2e module is
skipped unless `WRIT_E2E=1`; it builds nothing and uses `WRIT_BIN`.

Tested with Python 3.14, langgraph 1.2.12 / langchain-core 1.6.4,
openai-agents 0.22.3, claude-agent-sdk 0.2.157. Requires Python >= 3.10.

## License

Apache-2.0. See [LICENSE](LICENSE).
