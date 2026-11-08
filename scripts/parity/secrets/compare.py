import collections
import json
import pathlib
import re
import sys

SECRET_RULE = "committed-secret"
OWN_RULES = {"connection-string-password"}
KIND_PATTERN = re.compile(r"\(([A-Za-z0-9._-]+)\)")
DIFFERENCES_SHOWN = 15


def load_problems(path):
    text = path.read_text(encoding="utf-8")
    problems = []
    decoder = json.JSONDecoder()
    position = 0
    while position < len(text):
        chunk, position = decoder.raw_decode(text, position)
        problems.extend(chunk if isinstance(chunk, list) else [chunk])
        while position < len(text) and text[position].isspace():
            position += 1
    return problems


def gitleaks_findings(folder, name):
    report = folder / f"{name}.json"
    if not report.exists():
        return collections.Counter()
    prefix = f"/c/{name}/"
    return collections.Counter(
        (name, finding["File"].replace(prefix, "", 1), max(finding["StartLine"], 1), finding["RuleID"])
        for finding in json.loads(report.read_text(encoding="utf-8") or "[]")
    )


def kerbo_findings(folder, name):
    counts = collections.Counter()
    for problem in load_problems(folder / f"kerbo-{name}.json"):
        if problem["rule"] != SECRET_RULE:
            continue
        kind = KIND_PATTERN.search(problem["message"]).group(1)
        if kind in OWN_RULES:
            continue
        counts[(name, problem["path"].replace("\\", "/").removeprefix("./"), problem["line"], kind)] += 1
    return counts


def main():
    folder = pathlib.Path(sys.argv[1])
    expected, found = collections.Counter(), collections.Counter()
    for name in sys.argv[2:]:
        expected += gitleaks_findings(folder, name)
        found += kerbo_findings(folder, name)
    missing, extra = expected - found, found - expected
    print(f"gitleaks {sum(expected.values())} · kerbo {sum(found.values())} · coinciden {sum((expected & found).values())}")
    print(f"faltan {sum(missing.values())} · sobran {sum(extra.values())}")
    for label, difference in (("falta", missing), ("sobra", extra)):
        for key in list(difference)[:DIFFERENCES_SHOWN]:
            print(f"  {label} {key}")


main()
