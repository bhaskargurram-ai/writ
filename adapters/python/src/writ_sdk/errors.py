"""Exception hierarchy.

Every exception here means the same thing to a caller: **do not run the tool**
(or, after it ran, do not hand its output to the model). There is no error
path in this package that allows a dispatch.
"""

from __future__ import annotations

from typing import TYPE_CHECKING, Any

if TYPE_CHECKING:
    from .types import Decision


class WritError(Exception):
    """Base class. The tool call must not dispatch."""


class WritBlocked(WritError):
    """The policy did not authorize the call (deny, rejected or unresolved ask)."""

    def __init__(self, decision: Decision, message: str | None = None) -> None:
        self.decision = decision
        super().__init__(message or decision.describe())


class WritDenied(WritBlocked):
    """A `deny` verdict (or a final decision after `resolve` that does not dispatch)."""


class WritApprovalRejected(WritBlocked):
    """An `ask` verdict that the approver rejected, timed out on, or never saw."""


class WritUnavailable(WritError):
    """The writ binary is missing, cannot be started, or has crashed too often."""


class WritGatewayCrashed(WritError):
    """The `writ check` process exited while a request was in flight."""


class WritTimeout(WritError):
    """The gateway did not answer within the configured timeout."""


class WritProtocolError(WritError):
    """The gateway sent a malformed, mismatched or self-contradictory line."""


class WritGatewayError(WritError):
    """The gateway answered with an `error` object."""

    def __init__(self, code: str, message: str, raw: dict[str, Any] | None = None) -> None:
        self.code = code
        self.gateway_message = message
        self.raw = raw or {}
        super().__init__(f"writ error {code}: {message}")


class WritClosed(WritError):
    """The client was closed."""
