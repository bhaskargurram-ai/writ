# writ adapter: LangGraph

Implemented in the Python package [`writ-agent`](../python/) as
`writ_agent.langgraph`:

```bash
pip install "writ-agent[langgraph]"
```

```python
from writ_agent import Writ
from writ_agent.langgraph import writ_tool_node

graph.add_node("tools", writ_tool_node(tools, Writ()))  # instead of ToolNode(tools)
```

Every tool call goes through `ToolNode(wrap_tool_call=...)`: writ decides
before the tool runs; a blocked call becomes an error `ToolMessage` with
writ's reason. See [`../python/README.md`](../python/README.md).
