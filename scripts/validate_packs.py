#!/usr/bin/env python3
"""Compile every policy pack and example policy with `writ doctor`.

Pack files carry only `rules`, so each is wrapped in a minimal policy header
before compiling. Exits 1 if any file fails to compile.

Usage: python3 scripts/validate_packs.py [--writ PATH]
Builds `writ` with cargo first unless --writ points at an existing binary.
Requires PyYAML.
"""

import pathlib
import subprocess
import sys
import tempfile

import yaml

ROOT = pathlib.Path(__file__).resolve().parent.parent


def writ_binary(argv):
    if "--writ" in argv:
        return pathlib.Path(argv[argv.index("--writ") + 1])
    subprocess.run(["cargo", "build", "-q", "-p", "writ-cli"], cwd=ROOT, check=True)
    exe = ROOT / "target" / "debug" / "writ"
    return exe.with_suffix(".exe") if sys.platform == "win32" else exe


def main(argv):
    exe = writ_binary(argv)
    targets = sorted(ROOT.glob("packs/*/pack.yaml")) + sorted(ROOT.glob("examples/*.yaml"))
    ok = True
    with tempfile.TemporaryDirectory(prefix="writ-packval-") as tmp:
        tmp = pathlib.Path(tmp)
        for path in targets:
            doc = yaml.safe_load(path.read_text(encoding="utf-8"))
            if "default" not in doc:  # pack file -> wrap into a policy
                doc = {"version": 1, "default": "ask", "rules": doc["rules"]}
            probe = tmp / f"{path.parent.name}_{path.name}"
            probe.write_text(yaml.safe_dump(doc), encoding="utf-8")
            out = subprocess.run(
                [str(exe), "doctor", "--policy", str(probe), "--ledger", str(tmp / "none.jsonl")],
                capture_output=True,
                text=True,
            ).stdout
            line = next((l for l in out.splitlines() if l.startswith("policy")), "")
            good = bool(line) and "FAILED" not in line
            ok &= good
            rel = path.relative_to(ROOT).as_posix()
            print("PASS" if good else "FAIL", rel, "->", line.strip()[:90] or "(no policy line)")
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
