#!/usr/bin/env python
"""Compara una inspección de Qodana con la regla de kerbo que la calca, por archivo, línea y columna.

    python scripts/parity/java/compare.py junit4
    python scripts/parity/java/compare.py junit4 AutoCloseableResource auto-closeable-resource
"""

import collections
import json
import pathlib
import sys

PARITY = pathlib.Path(__file__).resolve().parents[4] / "kerbo-parity" / "qodana"
DEFAULT_RULES = ("RedundantThrows", "redundant-throws")
SHOWN = 40


def qodana_findings(repo, rule):
    results = json.loads((PARITY / repo / f"results-{rule}" / "qodana.sarif.json").read_text(encoding="utf-8"))
    found = {}
    for result in results["runs"][0].get("results", []):
        if result.get("ruleId") == rule:
            location = result["locations"][0]["physicalLocation"]
            key = (location["artifactLocation"]["uri"], location["region"]["startLine"], location["region"]["startColumn"])
            found[key] = result["message"]["text"]
    return found


def kerbo_findings(repo, rule):
    report = json.loads((PARITY / repo / "kerbo.json").read_text(encoding="utf-8"))
    return {
        (item["path"].removeprefix("./"), item["line"], item["column"]): item["message"]
        for item in report
        if item["rule"] == rule
    }


def show(title, findings):
    print(f"\n{title}: {len(findings)}")
    for key in sorted(findings)[:SHOWN]:
        print(f"  {key[0]}:{key[1]}:{key[2]}  {findings[key]}")


def main():
    repo = sys.argv[1]
    qodana_rule, kerbo_rule = sys.argv[2:4] if len(sys.argv) > 3 else DEFAULT_RULES
    qodana = qodana_findings(repo, qodana_rule)
    kerbo = kerbo_findings(repo, kerbo_rule)
    both = qodana.keys() & kerbo.keys()
    print(f"Qodana {len(qodana)}, kerbo {len(kerbo)}, en los dos {len(both)}")
    missing = {key: qodana[key] for key in qodana.keys() - kerbo.keys()}
    extra = {key: kerbo[key] for key in kerbo.keys() - qodana.keys()}
    print("faltan por carpeta:", collections.Counter("/".join(key[0].split("/")[:2]) for key in missing).most_common(5))
    show("sobran en kerbo", extra)
    show("faltan en kerbo", missing)


if __name__ == "__main__":
    main()
