"""writ-agent: policy-gated tool calls for Python agent frameworks.

Every tool call is sent to ``writ check --stdio`` (docs/INTERFACES.md,
Contract 6), checked against ``writ.yaml`` (allow / deny / ask / redact) and
recorded to writ's hash-chained ledger before the tool runs. Any failure to
get a clear "dispatch" answer from writ blocks the call.

Framework integrations live in submodules and import their framework only
when imported: ``writ_agent.langgraph``, ``writ_agent.openai_agents``,
``writ_agent.claude_agent_sdk``.
"""

from .client import AsyncWritClient, WritClient, find_writ_binary
from .errors import (
    WritApprovalRejected,
    WritBlocked,
    WritClosed,
    WritDenied,
    WritError,
    WritGatewayCrashed,
    WritGatewayError,
    WritProtocolError,
    WritTimeout,
    WritUnavailable,
)
from .guard import Writ, deny_all, get_default_writ, set_default_writ, to_text, writ_tool
from .toolmap import MappedTool, map_claude_tool
from .types import Approval, ApprovalRequest, Caller, Completion, Decision, Server, ToolCall

__version__ = "0.1.0"

__all__ = [
    "Approval",
    "ApprovalRequest",
    "AsyncWritClient",
    "Caller",
    "Completion",
    "Decision",
    "MappedTool",
    "Server",
    "ToolCall",
    "Writ",
    "WritApprovalRejected",
    "WritBlocked",
    "WritClient",
    "WritClosed",
    "WritDenied",
    "WritError",
    "WritGatewayCrashed",
    "WritGatewayError",
    "WritProtocolError",
    "WritTimeout",
    "WritUnavailable",
    "deny_all",
    "find_writ_binary",
    "get_default_writ",
    "map_claude_tool",
    "set_default_writ",
    "to_text",
    "writ_tool",
]
