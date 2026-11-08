import collections
import json
import pathlib
import sys

HADOLINT_PREFIX = "DL"
PARSE_ERROR = "dl1000"
DIFFERENCES_SHOWN = 20


def concatenated_arrays(text):
    decoder = json.JSONDecoder()
    position = 0
    while position < len(text):
        array, position = decoder.raw_decode(text, position)
        yield from array
        while position < len(text) and text[position].isspace():
            position += 1


def hadolint_counts(folder):
    text = (folder / "hadolint.json").read_text(encoding="utf-8")
    return collections.Counter(
        (finding["file"], finding["line"], finding["code"].lower())
        for finding in concatenated_arrays(text)
        if finding["code"].startswith(HADOLINT_PREFIX)
    )


def kerbo_counts(folder, rules):
    text = (folder / "kerbo.json").read_text(encoding="utf-8")
    return collections.Counter(
        (problem["path"].replace("\\", "/"), problem["line"], problem["rule"])
        for problem in concatenated_arrays(text)
        if problem["rule"] in rules
    )


def main():
    folder = pathlib.Path(sys.argv[1])
    expected = hadolint_counts(folder)
    unparsable = {path for path, _, rule in expected if rule == PARSE_ERROR}
    expected = collections.Counter({key: amount for key, amount in expected.items() if key[0] not in unparsable})
    found = kerbo_counts(folder, {rule for _, _, rule in expected})
    found = collections.Counter({key: amount for key, amount in found.items() if key[0] not in unparsable})
    print(f"sin comparar, hadolint no los parsea: {len(unparsable)} archivos")
    missing, extra = expected - found, found - expected
    print(f"hadolint {sum(expected.values())} · kerbo {sum(found.values())} · coinciden {sum((expected & found).values())}")
    print(f"faltan {sum(missing.values())} · sobran {sum(extra.values())}")
    for label, difference in (("falta", missing), ("sobra", extra)):
        by_rule = collections.Counter()
        for (_, _, rule), amount in difference.items():
            by_rule[rule] += amount
        for rule, amount in by_rule.most_common():
            print(f"  {label} {rule}: {amount}")
        for key in list(difference)[:DIFFERENCES_SHOWN]:
            print(f"    {label} {key}")


main()
