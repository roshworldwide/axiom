#!/usr/bin/env python3
"""Fail if the README's headline numbers drift from METRICS.md.

METRICS.md is the single source of truth. It ends with a HEADLINE block of
canonical phrases; this check asserts every one appears verbatim in README.md, so
the two can never silently disagree on a count. Run locally or in CI:

    python3 scripts/check_metrics.py
"""
import pathlib
import re
import sys

root = pathlib.Path(__file__).resolve().parent.parent
metrics = (root / "METRICS.md").read_text()
readme = (root / "README.md").read_text()

block = re.search(r"<!-- HEADLINE-START -->(.*?)<!-- HEADLINE-END -->", metrics, re.S)
if not block:
    sys.exit("METRICS.md: HEADLINE-START/END block not found")

phrases = [
    line.strip()[1:].strip()
    for line in block.group(1).splitlines()
    if line.strip().startswith("-")
]
if not phrases:
    sys.exit("METRICS.md: HEADLINE block is empty")

missing = [p for p in phrases if p not in readme]
if missing:
    print("README.md is out of sync with METRICS.md — missing canonical phrases:")
    for p in missing:
        print(f"  MISSING: {p!r}")
    print("\nFix README.md (or METRICS.md) so they agree; do not edit prose to")
    print("match prose — regenerate the real number first (see METRICS.md).")
    sys.exit(1)

print(f"OK: all {len(phrases)} headline metrics from METRICS.md are present in README.md")
