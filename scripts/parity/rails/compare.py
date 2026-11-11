"""kerbo (out/<repo>.json) contra squawk sobre el SQL que ActiveRecord manda al correr cada migración
(.parity/rails/<repo>/squawk.json).

Se compara por migración, regla y cantidad de avisos, sólo en las migraciones que reference.rb pudo
correr (las que fallan quedan en errores.txt y no tienen .sql): squawk marca líneas del SQL generado y
kerbo las de la llamada que lo genera, así que la línea no se puede cruzar.

Uso: python compare.py <repo>
"""
import collections
import json
import os
import pathlib
import re
import sys

HERE = pathlib.Path(__file__).resolve().parent
REPO = HERE.parent.parent.parent
PARITY = pathlib.Path(os.environ.get("KERBO_PARITY", REPO / ".parity"))
REFERENCE_PREFIX = "/out/"
MIGRATIONS_DIRECTORY = "migrate"
MIGRATION_NAME = re.compile(r"\A\d+_[_a-z0-9]*")


def reference(repo):
    counts = collections.Counter()
    for warning in json.loads((PARITY / "rails" / repo / "squawk.json").read_text(encoding="utf-8")):
        name = pathlib.PurePosixPath(warning["file"].removeprefix(REFERENCE_PREFIX)).stem
        counts[(name, warning["rule_name"])] += 1
    return counts


def converted_migrations(repo):
    return {path.stem for path in (PARITY / "rails" / repo).glob("*.sql")}


def migration_name(file_name):
    match = MIGRATION_NAME.match(file_name)
    return match.group() if match else file_name


def kerbo(repo, rules):
    counts = collections.Counter()
    for finding in json.loads((HERE / "out" / f"{repo}.json").read_text(encoding="utf-8")):
        path = pathlib.PurePosixPath(finding["path"].removeprefix("./"))
        if finding["rule"] in rules and path.parent.name == MIGRATIONS_DIRECTORY:
            counts[(migration_name(path.name), finding["rule"])] += 1
    return counts


def migration_rules():
    source = (REPO / "src" / "rules" / "families.rs").read_text(encoding="utf-8")
    block = source.split("rule_family!(MigrationRule {", 1)[1].split("});", 1)[0]
    return {line.split('"')[1] for line in block.splitlines() if '"' in line}


def main():
    repo = sys.argv[1]
    converted = converted_migrations(repo)
    expected = collections.Counter({key: count for key, count in reference(repo).items() if key[0] in converted})
    found = kerbo(repo, {rule for _, rule in expected} | migration_rules())
    found = collections.Counter({key: count for key, count in found.items() if key[0] in converted})
    differences = sorted(key for key in expected.keys() | found.keys() if expected[key] != found[key])
    for migration, rule in differences:
        print(f"{migration:60} {rule:40} squawk {expected[(migration, rule)]:3} kerbo {found[(migration, rule)]:3}")
    matched = sum(min(expected[key], found[key]) for key in expected.keys() | found.keys())
    print(f"squawk {sum(expected.values())}, kerbo {sum(found.values())}, coinciden {matched}, diferencias {len(differences)}")


if __name__ == "__main__":
    main()
