"""Cruza el SQL de kerbo (dump.sh) con el de sqlmigrate (reference.sh), sentencia por sentencia.

Uso: python diff.py <repo> [migración]   p. ej. python diff.py saleor product/0016_auto_20161207_0843
Sin migración, lista las que difieren; con ella, muestra las sentencias de cada lado que faltan en el otro.
"""
import os
import pathlib
import re
import sys

HERE = pathlib.Path(__file__).resolve().parent
PARITY = pathlib.Path(os.environ.get("KERBO_PARITY", HERE.parent.parent.parent / ".parity"))
COMMENT = "--"
TRANSACTION = {"BEGIN", "COMMIT"}
VOLATILE_DEFAULT = re.compile(r"'[^']*'::(timestamptz|date|time|uuid)")


def statements(path):
    if not path.exists():
        return []
    lines = [line for line in path.read_text(encoding="utf-8").splitlines() if not line.startswith(COMMENT)]
    text = VOLATILE_DEFAULT.sub(r"'?'::\1", " ".join(lines))
    found = [statement.strip() for statement in text.split(";")]
    return [statement for statement in found if statement and statement not in TRANSACTION]


def pair(repo, migration):
    reference = statements(PARITY / "django" / repo / f"{migration}.sql")
    kerbo = statements(PARITY / "django" / f"{repo}-kerbo" / f"{migration}.sql")
    return reference, kerbo


def differs(reference, kerbo):
    return sorted(reference) != sorted(kerbo)


def migrations(repo):
    root = PARITY / "django" / repo
    return sorted(str(path.relative_to(root).with_suffix("")).replace("\\", "/") for path in root.glob("*/*.sql"))


def main():
    repo = sys.argv[1]
    if len(sys.argv) > 2:
        reference, kerbo = pair(repo, sys.argv[2])
        for statement in reference:
            if statement not in kerbo:
                print(f"- {statement}")
        for statement in kerbo:
            if statement not in reference:
                print(f"+ {statement}")
        return
    different = [migration for migration in migrations(repo) if differs(*pair(repo, migration))]
    for migration in different:
        print(migration)
    print(f"{len(different)} de {len(migrations(repo))} migraciones con SQL distinto")


if __name__ == "__main__":
    main()
