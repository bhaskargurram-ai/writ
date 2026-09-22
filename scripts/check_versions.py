#!/usr/bin/env python3
"""Fail unless every released artifact carries the same version.

Checks the Rust workspace (writ-cli wheel and binaries), the writ-sdk Python
package and its writ-cli pin, the @writ-agent/sdk npm package and its
@writ-agent/cli pin, and @writ-agent/cli with its platform-package pins.

Usage: python3 scripts/check_versions.py [vX.Y.Z]   (tag optional)
"""

import json
import re
import sys
import tomllib
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent


def main(argv):
    cargo = tomllib.loads((ROOT / "Cargo.toml").read_text(encoding="utf-8"))
    rust = cargo["workspace"]["package"]["version"]
    py = tomllib.loads((ROOT / "adapters/python/pyproject.toml").read_text(encoding="utf-8"))
    sdk = json.loads((ROOT / "adapters/typescript/package.json").read_text(encoding="utf-8"))
    cli = json.loads((ROOT / "packaging/npm/cli/package.json").read_text(encoding="utf-8"))

    found = {
        "Cargo.toml workspace.package.version": rust,
        "writ-sdk version": py["project"]["version"],
        "@writ-agent/sdk version": sdk["version"],
        "@writ-agent/cli version": cli["version"],
    }
    for dep in py["project"].get("dependencies", []):
        m = re.fullmatch(r"writ-cli==(\S+)", dep.replace(" ", ""))
        if m:
            found["writ-sdk -> writ-cli pin"] = m.group(1)
    found["@writ-agent/sdk -> @writ-agent/cli pin"] = sdk.get("optionalDependencies", {}).get("@writ-agent/cli")
    for name, ver in cli.get("optionalDependencies", {}).items():
        found[f"@writ-agent/cli -> {name} pin"] = ver
    if len(argv) > 0:
        found["git tag"] = argv[0].removeprefix("refs/tags/").removeprefix("v")

    expected = rust
    bad = {k: v for k, v in found.items() if v != expected}
    for k, v in found.items():
        print(f"{'ok ' if v == expected else 'BAD'} {k}: {v}")
    if bad:
        print(f"\nversion mismatch: expected {expected} everywhere", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
