#!/usr/bin/env python
"""Compara las inspecciones de Qodana que tienen par en una regla de kerbo (imports sin usar y
modificadores redundantes) con kerbo imitando a Checkstyle y a PMD, por regla, archivo y línea.
Qodana avisa una vez por modificador y kerbo también, pero las columnas no coinciden: cuenta una
vez cada regla en cada línea.

    scripts/parity/java/qodana.sh junit4 UNUSED_IMPORT,UnnecessaryModifier,FinalPrivateMethod,FinalStaticMethod,FinalMethodInFinalClass
    python scripts/parity/java/qodana_rules.py junit4

Necesita el SARIF de esa corrida (results-UNUSED_IMPORT) y kerbo.json y kerbo-pmd.json de kerbo.sh.
"""

import collections
import json
import os
import pathlib
import sys

PARITY = pathlib.Path(os.environ.get("KERBO_PARITY", pathlib.Path(__file__).resolve().parents[3] / ".parity")) / "qodana"
INSPECTIONS = {
    "UNUSED_IMPORT": "unused-import",
    "UnnecessaryModifier": "redundant-modifier",
    "UnnecessaryInterfaceModifier": "redundant-modifier",
    "UnnecessaryEnumModifier": "redundant-modifier",
    "UnnecessaryRecordModifier": "redundant-modifier",
    "FinalPrivateMethod": "redundant-modifier",
    "FinalStaticMethod": "redundant-modifier",
    "FinalMethodInFinalClass": "redundant-modifier",
}
RESULTS = "results-UNUSED_IMPORT"
REPORTS = {"Checkstyle": "kerbo.json", "PMD": "kerbo-pmd.json"}
UNANALYZED = {"guava": ("android/",)}
SHOWN = int(os.environ.get("SHOWN", "15"))


def qodana_findings(repo):
    results = json.loads((PARITY / repo / RESULTS / "qodana.sarif.json").read_text(encoding="utf-8"))
    found = collections.defaultdict(dict)
    for result in results["runs"][0].get("results", []):
        rule = INSPECTIONS.get(result["ruleId"])
        if rule:
            location = result["locations"][0]["physicalLocation"]
            key = (location["artifactLocation"]["uri"], location["region"]["startLine"])
            found[rule][key] = f"{result['ruleId']}: {result['message']['text']}"
    return found


def kerbo_findings(repo, report):
    found = collections.defaultdict(set)
    for item in json.loads((PARITY / repo / report).read_text(encoding="utf-8")):
        path = item["path"].removeprefix("./")
        if item["rule"] in INSPECTIONS.values() and not path.startswith(UNANALYZED.get(repo, ())):
            found[item["rule"]].add((path, item["line"]))
    return found


def main():
    sys.stdout.reconfigure(encoding="utf-8")
    repo = sys.argv[1]
    reference = qodana_findings(repo)
    for imitated, report in REPORTS.items():
        kerbo = kerbo_findings(repo, report)
        for rule in sorted(set(INSPECTIONS.values())):
            missing = sorted(set(reference[rule]) - kerbo[rule])
            extra = sorted(kerbo[rule] - set(reference[rule]))
            print(f"{rule} imitando a {imitated}: Qodana {len(reference[rule])}, faltan {len(missing)}, sobran {len(extra)}")
            for path, line in missing[:SHOWN]:
                print(f"    faltan {path}:{line} {reference[rule][(path, line)]}")
            for path, line in extra[:SHOWN]:
                print(f"    sobran {path}:{line}")


if __name__ == "__main__":
    main()
