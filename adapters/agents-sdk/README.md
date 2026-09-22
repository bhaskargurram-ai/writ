# writ adapter: OpenAI Agents SDK

Implemented in the Python package [`writ-agent`](../python/) as
`writ_agent.openai_agents`:

```bash
pip install "writ-agent[openai-agents]"
```

```python
from writ_agent import Writ
from writ_agent.openai_agents import guard_agent

agent = guard_agent(agent, Writ())
```

Each `FunctionTool`'s `on_invoke_tool` is wrapped: writ decides before the
tool body runs, and a blocked call returns writ's reason as the tool result.
See [`../python/README.md`](../python/README.md) for why this extension point
and not `RunHooks` or tool guardrails.
