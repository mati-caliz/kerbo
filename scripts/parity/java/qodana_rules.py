#!/usr/bin/env python
"""Compara las inspecciones de Qodana que tienen par en una regla de kerbo (imports sin usar,
modificadores redundantes, escapes Unicode de más y la API nueva) con kerbo imitando a Checkstyle y a PMD, por
regla, archivo y línea. Las columnas no coinciden (Qodana cuenta caracteres, kerbo bytes): de los
modificadores cuenta una vez cada línea, y de los escapes, cuántos hay en cada línea.

    scripts/parity/java/qodana.sh junit4 UNUSED_IMPORT,UnnecessaryModifier,FinalPrivateMethod,FinalStaticMethod,FinalMethodInFinalClass
    python scripts/parity/java/qodana_rules.py junit4
    python scripts/parity/java/qodana_rules.py guava results-SequencedCollectionMethodCanBeUsed

Necesita el SARIF de esa corrida (results-UNUSED_IMPORT si no se dice otro) y kerbo.json y
kerbo-pmd.json de kerbo.sh.
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
    "UnnecessaryUnicodeEscape": "unnecessary-unicode-escape",
    "Java8ListSort": "java8-list-sort",
    "SizeReplaceableByIsEmpty": "size-replaceable-by-is-empty",
}
COUNTED_PER_LINE = {"unnecessary-unicode-escape"}
RESULTS = "results-UNUSED_IMPORT"
REPORTS = {"Checkstyle": "kerbo.json", "PMD": "kerbo-pmd.json"}
UNANALYZED = {"guava": ("android/",)}
SHOWN = int(os.environ.get("SHOWN", "15"))


def qodana_findings(repo, results_folder):
    results = json.loads((PARITY / repo / results_folder / "qodana.sarif.json").read_text(encoding="utf-8"))
    found = collections.defaultdict(collections.Counter)
    messages = {}
    for result in results["runs"][0].get("results", []):
        rule = INSPECTIONS.get(result["ruleId"])
        if rule:
            location = result["locations"][0]["physicalLocation"]
            key = (location["artifactLocation"]["uri"], location["region"]["startLine"])
            found[rule][key] = found[rule][key] + 1 if rule in COUNTED_PER_LINE else 1
            messages[key] = f"{result['ruleId']}: {result['message']['text']}"
    return found, messages


def kerbo_findings(repo, report):
    found = collections.defaultdict(collections.Counter)
    for item in json.loads((PARITY / repo / report).read_text(encoding="utf-8")):
        path = item["path"].removeprefix("./")
        rule = item["rule"]
        if rule in INSPECTIONS.values() and not path.startswith(UNANALYZED.get(repo, ())):
            key = (path, item["line"])
            found[rule][key] = found[rule][key] + 1 if rule in COUNTED_PER_LINE else 1
    return found


def main():
    sys.stdout.reconfigure(encoding="utf-8")
    repo = sys.argv[1]
    reference, messages = qodana_findings(repo, sys.argv[2] if len(sys.argv) > 2 else RESULTS)
    for imitated, report in REPORTS.items():
        kerbo = kerbo_findings(repo, report)
        for rule in sorted(set(INSPECTIONS.values())):
            if not reference[rule]:
                continue
            missing = reference[rule] - kerbo[rule]
            extra = kerbo[rule] - reference[rule]
            total = sum(reference[rule].values())
            print(f"{rule} imitando a {imitated}: Qodana {total}, faltan {missing.total()}, sobran {extra.total()}")
            for (path, line), count in sorted(missing.items())[:SHOWN]:
                print(f"    faltan {path}:{line} x{count} {messages[(path, line)]}")
            for (path, line), count in sorted(extra.items())[:SHOWN]:
                print(f"    sobran {path}:{line} x{count}")


if __name__ == "__main__":
    main()
