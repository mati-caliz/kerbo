"""kerbo (out/<repo>.json) contra squawk sobre el SQL de `sqlmigrate` (.parity/django/<repo>/squawk.json).

Las migraciones se nombran por el label de su app (el de apps.py, como en sqlmigrate), no por su carpeta.
Se compara por migración, regla y cantidad de avisos, sólo en las migraciones que `sqlmigrate`
pudo convertir (las que fallan quedan en errores.txt y no tienen referencia) y que son de apps del
corpus (las de paquetes instalados, como auth en sentry, kerbo no las ve): squawk marca líneas
del SQL generado y kerbo las de la operación que lo genera, así que la línea no se puede cruzar.

Uso: python compare.py <repo>
     python compare.py --apps-root <repo>
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
APPS_ROOTS = {
    "django": "django/django/contrib",
    "saleor": "saleor/saleor",
    "zulip": "zulip",
    "wagtail": "wagtail",
    "sentry": "sentry/src",
}
REFERENCE_PREFIX = "/out/"
MIGRATIONS_DIRECTORY = "migrations"
LABEL = re.compile(r"""^\s*label\s*=\s*["']([^"']+)["']""", re.MULTILINE)


def label(repo, app_directory):
    config = PARITY / "corpus" / APPS_ROOTS[repo] / app_directory / "apps.py"
    found = LABEL.search(config.read_text(encoding="utf-8")) if config.exists() else None
    return found.group(1) if found else app_directory.name


def reference(repo):
    counts = collections.Counter()
    for warning in json.loads((PARITY / "django" / repo / "squawk.json").read_text(encoding="utf-8")):
        app, name = warning["file"].removeprefix(REFERENCE_PREFIX).split("/")
        counts[(f"{app}/{pathlib.PurePosixPath(name).stem}", warning["rule_name"])] += 1
    return counts


def corpus_apps(repo):
    root = PARITY / "corpus" / APPS_ROOTS[repo]
    return {label(repo, path.parent.relative_to(root)) for path in root.rglob(MIGRATIONS_DIRECTORY) if path.is_dir()}


def converted_migrations(repo):
    root = PARITY / "django" / repo
    apps = corpus_apps(repo)
    return {f"{path.parent.name}/{path.stem}" for path in root.glob("*/*.sql") if path.parent.name in apps}


def kerbo(repo, rules):
    prefix = APPS_ROOTS[repo] + "/"
    counts = collections.Counter()
    for finding in json.loads((HERE / "out" / f"{repo}.json").read_text(encoding="utf-8")):
        path = pathlib.PurePosixPath(finding["path"].removeprefix("./").removeprefix(prefix))
        if finding["rule"] in rules and path.parent.name == MIGRATIONS_DIRECTORY:
            counts[(f"{label(repo, path.parent.parent)}/{path.stem}", finding["rule"])] += 1
    return counts


def main():
    if sys.argv[1] == "--apps-root":
        print(APPS_ROOTS[sys.argv[2]])
        return
    repo = sys.argv[1]
    converted = converted_migrations(repo)
    expected = collections.Counter({key: count for key, count in reference(repo).items() if key[0] in converted})
    found = kerbo(repo, {rule for _, rule in expected} | migration_rules())
    found = collections.Counter({key: count for key, count in found.items() if key[0] in converted})
    differences = sorted(key for key in expected.keys() | found.keys() if expected[key] != found[key])
    for migration, rule in differences:
        print(f"{migration:45} {rule:40} squawk {expected[(migration, rule)]:3} kerbo {found[(migration, rule)]:3}")
    matched = sum(min(expected[key], found[key]) for key in expected.keys() | found.keys())
    print(f"squawk {sum(expected.values())}, kerbo {sum(found.values())}, coinciden {matched}, diferencias {len(differences)}")


def migration_rules():
    source = (REPO / "src" / "rules" / "families.rs").read_text(encoding="utf-8")
    block = source.split("rule_family!(MigrationRule {", 1)[1].split("});", 1)[0]
    return {line.split('"')[1] for line in block.splitlines() if '"' in line}


if __name__ == "__main__":
    main()
