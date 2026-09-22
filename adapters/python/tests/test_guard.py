"""The framework-neutral gate: decide -> approve -> run -> complete."""

from __future__ import annotations

import time

import pytest

from writ_sdk import (
    Approval,
    Writ,
    WritApprovalRejected,
    WritDenied,
    WritError,
    WritProtocolError,
    WritUnavailable,
    set_default_writ,
    writ_tool,
)


class Spy:
    def __init__(self, result="ran"):
        self.calls = 0
        self.result = result

    def __call__(self, *a, **kw):
        self.calls += 1
        return self.result


def ops(log):
    return [r["op"] for r in log()]


def test_allow_runs_and_completes(fake_bin, fake_log):
    spy = Spy()
    with Writ(session_id="sess") as w:
        assert w.execute("allowed", {"x": 1}, spy) == "ran"
    assert spy.calls == 1
    log = fake_log()
    assert ops(fake_log) == ["decide", "complete"]
    assert log[0]["call"]["session_id"] == "sess" and log[0]["call"]["args"] == {"x": 1}
    assert log[1]["output"] == "ran" and log[1]["ok"] is True


def test_deny_never_runs(fake_bin, fake_log):
    spy = Spy()
    with Writ() as w, pytest.raises(WritDenied) as ei:
        w.execute("denied", {}, spy)
    assert spy.calls == 0
    assert ei.value.decision.rule_id == "no-rm"
    assert ops(fake_log) == ["decide"]


def test_ask_default_approver_rejects(fake_bin, fake_log):
    spy = Spy()
    with Writ(ask="defer") as w, pytest.raises(WritApprovalRejected):
        w.execute("asky", {}, spy)
    assert spy.calls == 0
    log = fake_log()
    assert [r["op"] for r in log] == ["decide", "resolve"] and log[1]["approved"] is False


def test_ask_in_deny_mode_fails_closed(fake_bin, fake_log):
    spy = Spy()
    with Writ(approver=lambda r: True, ask="deny") as w, pytest.raises(WritApprovalRejected):
        w.execute("asky", {}, spy)
    assert spy.calls == 0 and ops(fake_log) == ["decide"]


def test_ask_defer_approved(fake_bin, fake_log):
    seen = []

    def approver(req):
        seen.append(req)
        return Approval(True, "human:alice")

    spy = Spy()
    with Writ(approver=approver) as w:
        assert w.execute("asky", {"q": "DROP"}, spy) == "ran"
    assert spy.calls == 1
    assert seen[0].diff == "--- diff ---" and seen[0].decision.irreversible
    log = fake_log()
    assert [r["op"] for r in log] == ["decide", "resolve", "complete"]
    assert log[1]["approver"] == "human:alice"
    assert log[2]["ref"] == "ref-1.approved"  # complete uses the ref resolve returned


def test_ask_defer_rejected(fake_bin):
    spy = Spy()
    with Writ(approver=lambda r: False) as w, pytest.raises(WritApprovalRejected):
        w.execute("asky", {}, spy)
    assert spy.calls == 0


@pytest.mark.parametrize("bad", [None, "yes", 1, lambda r: (_ for _ in ()).throw(RuntimeError("boom"))])
def test_ask_bad_approver_rejects(fake_bin, bad):
    approver = bad if callable(bad) else (lambda r, b=bad: b)
    spy = Spy()
    with Writ(approver=approver) as w, pytest.raises(WritApprovalRejected):
        w.execute("asky", {}, spy)
    assert spy.calls == 0


def test_ask_approver_timeout_rejects(fake_bin):
    spy = Spy()
    with Writ(approver=lambda r: time.sleep(5) or True, approval_timeout=0.3) as w:
        with pytest.raises(WritApprovalRejected):
            w.execute("asky", {}, spy)
    assert spy.calls == 0


def test_redact_returns_redacted(fake_bin):
    with Writ() as w:
        out = w.execute("redacty", {}, lambda: {"ssn": "123-45-6789"})
    assert out == '{"ssn": "[redacted-by-writ]"}'


def test_redact_without_output_withholds(fake_bin):
    with Writ() as w, pytest.raises(WritProtocolError):
        w.execute("redact_noout", {}, lambda: "123-45-6789")


def test_tool_exception_is_recorded_and_reraised(fake_bin, fake_log):
    def boom():
        raise ValueError("nope")

    with Writ() as w, pytest.raises(ValueError):
        w.execute("allowed", {}, boom)
    log = fake_log()
    assert log[-1]["op"] == "complete" and log[-1]["ok"] is False and log[-1]["exit"] == 1


@pytest.mark.parametrize("tool", ["malformed", "hang", "crash", "error", "wrongid", "liar"])
def test_gateway_failures_never_run(fake_bin, tool):
    spy = Spy()
    with Writ(timeout=0.5) as w, pytest.raises(WritError):
        w.execute(tool, {}, spy)
    assert spy.calls == 0


def test_missing_binary_never_runs(monkeypatch, tmp_path):
    monkeypatch.setenv("WRIT_BIN", str(tmp_path / "missing" / "writ"))
    spy = Spy()
    with Writ() as w, pytest.raises(WritUnavailable):
        w.execute("allowed", {}, spy)
    assert spy.calls == 0


def test_guarded_binds_arguments(fake_bin, fake_log):
    with Writ() as w:

        @w.tool("allowed")
        def read_file(path: str, limit: int = 10) -> str:
            return f"{path}:{limit}"

        assert read_file("a.txt") == "a.txt:10"
        g = w.guarded(lambda command: "x", name="bash")
        with pytest.raises(WritDenied):
            g(command="rm -rf /")
    assert fake_log()[0]["call"]["args"] == {"path": "a.txt", "limit": 10}


async def test_guarded_async(fake_bin):
    async def approver(req):
        return True

    async with Writ(approver=approver) as w:

        @w.tool("asky")
        async def migrate(sql: str) -> str:
            return "done"

        assert await migrate("ALTER") == "done"

        @w.tool("denied")
        async def nope() -> str:  # pragma: no cover - must not run
            raise AssertionError("ran")

        with pytest.raises(WritDenied):
            await nope()


def test_writ_tool_default_is_lazy(fake_bin, fake_log):
    @writ_tool("allowed")
    def f(a):
        return a * 2

    assert fake_log() == []  # decorating started nothing
    w = Writ()
    set_default_writ(w)
    try:
        assert f(3) == 6
    finally:
        set_default_writ(None)
        w.close()
