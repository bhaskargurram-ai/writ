"""End to end against the real `writ` binary. Opt in: WRIT_E2E=1 and WRIT_BIN=/path/to/writ.

Builds nothing. Writes a writ.yaml, runs allow / deny / redact / ask through
`writ check --stdio`, then asserts `writ verify` accepts the ledger.
"""

from __future__ import annotations

import json
import os
import subprocess
import textwrap

import pytest

from writ_sdk import Approval, Writ, WritApprovalRejected, WritDenied, find_writ_binary

pytestmark = pytest.mark.skipif(
    os.environ.get("WRIT_E2E") != "1", reason="set WRIT_E2E=1 (and WRIT_BIN) to run against the real writ binary"
)

POLICY = textwrap.dedent(
    r"""
    version: 1
    default: deny
    rules:
      - id: no-rm
        when: tool == "bash" and command matches "rm -rf"
        verdict: deny
        reason: "Destructive command."
      - id: pii
        when: tool == "lookup"
        verdict: redact
        patterns:
          - "\\b\\d{3}-\\d{2}-\\d{4}\\b"
      - id: prod-migration
        when: tool == "migrate"
        verdict: ask
        timeout: 1m
        reason: "Schema change. A human signs this one."
      - id: reads
        when: tool == "fs.read" or tool == "bash"
        verdict: allow
    """
)


@pytest.fixture
def workspace(tmp_path):
    (tmp_path / "writ.yaml").write_text(POLICY, encoding="utf-8")
    return tmp_path


def writ_argv() -> list[str]:
    return find_writ_binary()


def test_real_gateway_allow_deny_redact_ask_then_verify(workspace):
    policy, ledger = workspace / "writ.yaml", workspace / ".writ" / "ledger.jsonl"
    ran: list[str] = []

    def approver(req):
        return Approval(req.call.args.get("sql") == "ALTER TABLE t ADD c int", "human:e2e")

    with Writ(policy=str(policy), ledger=str(ledger), cwd=str(workspace), approver=approver, session_id="e2e") as w:
        assert w.execute("fs.read", {"path": "README.md"}, lambda: ran.append("read") or "hello") == "hello"

        with pytest.raises(WritDenied) as ei:
            w.execute("bash", {"command": "rm -rf /"}, lambda: ran.append("rm"))
        assert ei.value.decision.rule_id == "no-rm"
        assert ei.value.decision.location and ei.value.decision.location.startswith("writ.yaml:")

        out = w.execute("lookup", {"user": "bob"}, lambda: ran.append("lookup") or "bob 123-45-6789")
        assert "123-45-6789" not in out and "bob" in out

        assert w.execute("migrate", {"sql": "ALTER TABLE t ADD c int"}, lambda: ran.append("mig") or "ok") == "ok"
        with pytest.raises(WritApprovalRejected):
            w.execute("migrate", {"sql": "DROP TABLE t"}, lambda: ran.append("drop"))

        with pytest.raises(WritDenied):  # policy default: deny
            w.execute("unknown.tool", {}, lambda: ran.append("unknown"))

    assert ran == ["read", "lookup", "mig"]

    records = [json.loads(l) for l in ledger.read_text(encoding="utf-8").splitlines() if l.strip()]
    kinds = [str(r.get("kind", "")).lower() for r in records]
    assert kinds.count("decision") == 6 and kinds.count("execution") == 3

    verify = subprocess.run(
        [*writ_argv(), "--ledger", str(ledger), "verify"], cwd=workspace, capture_output=True, text=True
    )
    assert verify.returncode == 0, verify.stdout + verify.stderr


def test_real_gateway_ask_deny_mode_fails_closed(workspace):
    ran: list[str] = []
    with Writ(policy=str(workspace / "writ.yaml"), ledger=str(workspace / "l.jsonl"), cwd=str(workspace)) as w:
        with pytest.raises(WritApprovalRejected):
            w.execute("migrate", {"sql": "ALTER"}, lambda: ran.append("x"))
    assert ran == []
