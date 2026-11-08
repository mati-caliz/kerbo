import collections
import json
import sys
import pathlib

MIGRATION_RULES_LIMIT = 15
SYNTAX_ERROR = "syntax-error"


def concatenated_arrays(text):
    decoder = json.JSONDecoder()
    position = 0
    while position < len(text):
        array, position = decoder.raw_decode(text, position)
        yield from array
        while position < len(text) and text[position].isspace():
            position += 1


def squawk_counts(folder):
    counts = collections.Counter()
    for warning in concatenated_arrays((folder / "squawk.json").read_text(encoding="utf-8")):
        path = warning["file"].replace("\\", "/")
        counts[(path, 0, warning["rule_name"])] += 1
    return counts


def kerbo_counts(folder, rules):
    counts = collections.Counter()
    for problem in concatenated_arrays((folder / "kerbo.json").read_text(encoding="utf-8")):
        if problem["rule"] in rules:
            counts[(problem["path"], 0, problem["rule"])] += 1
    return counts


def main():
    folder = pathlib.Path(sys.argv[1])
    expected = squawk_counts(folder)
    unparsable = {path for path, _, rule in expected if rule == SYNTAX_ERROR}
    expected = collections.Counter({key: amount for key, amount in expected.items() if key[0] not in unparsable})
    rules = {rule for _, _, rule in expected}
    found = kerbo_counts(folder, rules)
    found = collections.Counter({key: amount for key, amount in found.items() if key[0] not in unparsable})
    matching = sum((expected & found).values())
    missing = expected - found
    extra = found - expected
    print(f"squawk {sum(expected.values())} · kerbo {sum(found.values())} · coinciden {matching}")
    print(f"faltan {sum(missing.values())} · sobran {sum(extra.values())}")
    for label, difference in (("falta", missing), ("sobra", extra)):
        by_rule = collections.Counter()
        for (_, _, rule), amount in difference.items():
            by_rule[rule] += amount
        for rule, amount in by_rule.most_common():
            print(f"  {label} {rule}: {amount}")
        for (path, line, rule), _ in list(difference.items())[:MIGRATION_RULES_LIMIT]:
            print(f"    {label} {path}:{line} {rule}")


main()
