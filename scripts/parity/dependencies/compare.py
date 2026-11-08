import collections
import json
import pathlib
import re
import sys

OSV_RULE = "vulnerable-dependency"
CONTAINER_PREFIX = "/c/"
ADVISORY_PATTERN = re.compile(r"^`(.+?)` (\S+) (?:tiene|has) (\S+?)(?: \(([^)]*)\))?:")
DIFFERENCES_SHOWN = 25


def directory_of(path):
    return str(pathlib.PurePosixPath(path).parent)


def concatenated_values(text):
    decoder = json.JSONDecoder()
    position = 0
    while position < len(text):
        value, position = decoder.raw_decode(text, position)
        yield from value if isinstance(value, list) else [value]
        while position < len(text) and text[position].isspace():
            position += 1


def osv_scanner_groups(folder):
    report = json.loads((folder / "osv-scanner.json").read_text(encoding="utf-8") or "{}")
    groups = collections.defaultdict(list)
    for result in report.get("results", []):
        path = result["source"]["path"].removeprefix(CONTAINER_PREFIX)
        for entry in result["packages"]:
            package = entry["package"]
            for group in entry.get("groups", []):
                key = (directory_of(path), package["name"], package["version"])
                identifiers = set(group["ids"])
                if identifiers not in groups[key]:
                    groups[key].append(identifiers)
    return groups


def kerbo_groups(folder):
    groups = collections.defaultdict(list)
    for problem in concatenated_values((folder / "kerbo.json").read_text(encoding="utf-8")):
        if problem["rule"] != OSV_RULE:
            continue
        match = ADVISORY_PATTERN.match(problem["message"])
        if match is None:
            print(f"mensaje que no se entiende: {problem['path']}: {problem['message']}")
            continue
        package, version, identifier, aliases = match.groups()
        identifiers = {identifier, *(aliases.split(", ") if aliases else [])}
        key = (directory_of(problem["path"].replace("\\", "/")), package, version)
        if identifiers not in groups[key]:
            groups[key].append(identifiers)
    return groups


def unmatched(source, target):
    remaining = []
    for key, source_groups in source.items():
        available = list(target.get(key, []))
        for identifiers in source_groups:
            hit = next((candidate for candidate in available if candidate & identifiers), None)
            if hit is None:
                remaining.append((key, sorted(identifiers)[:3]))
            else:
                available.remove(hit)
    return remaining


def main():
    folder = pathlib.Path(sys.argv[1])
    expected, found = osv_scanner_groups(folder), kerbo_groups(folder)
    expected_total = sum(len(value) for value in expected.values())
    found_total = sum(len(value) for value in found.values())
    missing, extra = unmatched(expected, found), unmatched(found, expected)
    print(f"osv-scanner {expected_total} · kerbo {found_total} · coinciden {expected_total - len(missing)}")
    print(f"faltan {len(missing)} · sobran {len(extra)}")
    for label, difference in (("falta", missing), ("sobra", extra)):
        for key, identifiers in difference[:DIFFERENCES_SHOWN]:
            print(f"  {label} {key} {identifiers}")


main()
