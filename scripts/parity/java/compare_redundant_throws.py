#!/usr/bin/env python
"""Compara RedundantThrows de Qodana con redundant-throws de kerbo, por archivo, línea y columna.

    python scripts/parity/java/compare_redundant_throws.py junit4
"""

import collections
import json
import pathlib
import sys

PARITY = pathlib.Path(__file__).resolve().parents[4] / "kerbo-parity" / "qodana"
QODANA_RULE = "RedundantThrows"
KERBO_RULE = "redundant-throws"
SHOWN = 40


def qodana_findings(repo):
    results = json.loads((PARITY / repo / "results" / "qodana.sarif.json").read_text(encoding="utf-8"))
    found = {}
    for result in results["runs"][0].get("results", []):
        if result.get("ruleId") == QODANA_RULE:
            location = result["locations"][0]["physicalLocation"]
            key = (location["artifactLocation"]["uri"], location["region"]["startLine"], location["region"]["startColumn"])
            found[key] = result["message"]["text"]
    return found


def kerbo_findings(repo):
    report = json.loads((PARITY / repo / "kerbo.json").read_text(encoding="utf-8"))
    return {
        (item["path"].removeprefix("./"), item["line"], item["column"]): item["message"]
        for item in report
        if item["rule"] == KERBO_RULE
    }


def show(title, findings):
    print(f"\n{title}: {len(findings)}")
    for key in sorted(findings)[:SHOWN]:
        print(f"  {key[0]}:{key[1]}:{key[2]}  {findings[key]}")


def main():
    repo = sys.argv[1]
    qodana = qodana_findings(repo)
    kerbo = kerbo_findings(repo)
    both = qodana.keys() & kerbo.keys()
    print(f"Qodana {len(qodana)}, kerbo {len(kerbo)}, en los dos {len(both)}")
    missing = {key: qodana[key] for key in qodana.keys() - kerbo.keys()}
    extra = {key: kerbo[key] for key in kerbo.keys() - qodana.keys()}
    print("faltan por carpeta:", collections.Counter("/".join(key[0].split("/")[:2]) for key in missing).most_common(5))
    show("sobran en kerbo", extra)
    show("faltan en kerbo", missing)


if __name__ == "__main__":
    main()
