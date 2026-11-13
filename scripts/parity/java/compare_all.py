#!/usr/bin/env python
"""Compara todas las inspecciones de Qodana medidas en un repo con las reglas de kerbo que las calcan.

    python scripts/parity/java/compare_all.py commons-lang
"""

import os
import pathlib
import subprocess
import sys

PAIRS = [
    ("RedundantThrows", "redundant-throws"),
    ("AutoCloseableResource", "auto-closeable-resource"),
    ("DataFlowIssue", "data-flow-issue"),
    ("DataFlowIssue", "null-argument"),
    ("DataFlowIssue", "already-assigned"),
    ("DataFlowIssue", "failing-call"),
    ("DataFlowIssue", "nullable-return"),
]
COMPARE = pathlib.Path(__file__).with_name("compare.py")


def main():
    repo = sys.argv[1]
    for qodana, kerbo in PAIRS:
        print(f"== {qodana} / {kerbo}")
        result = subprocess.run(
            [sys.executable, str(COMPARE), repo, qodana, kerbo], capture_output=True, text=True, encoding="utf-8", env={**os.environ, "PYTHONIOENCODING": "utf-8"}
        )
        lines = result.stdout.splitlines()
        print("\n".join(lines[:1]))
        extra = [line for line in lines if line.startswith("sobran") or line.startswith("  ")]
        shown = []
        printing = False
        for line in lines:
            if line.startswith("sobran"):
                printing = True
            elif line.startswith("faltan en kerbo"):
                printing = False
            if printing:
                shown.append(line)
        print("\n".join(shown) if shown else "\n".join(extra[:0]))


if __name__ == "__main__":
    main()
