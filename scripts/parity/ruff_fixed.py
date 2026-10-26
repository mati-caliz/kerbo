"""Regenera los .fixed y .unsafe de los fixtures de Python con `ruff check --fix`.

Uso: python -I scripts/parity/ruff_fixed.py [--check]
Necesita ruff 0.17.1 en ../kerbo-parity/ruff/ruff.exe (al lado del repo).
Con --check no escribe nada: sólo lista los fixtures cuyo golden cambiaría.
"""
import pathlib
import re
import shutil
import subprocess
import sys
import tempfile

REPO = pathlib.Path(__file__).resolve().parent.parent.parent
PARITY = REPO.parent / "kerbo-parity"
RUFF = PARITY / "ruff" / "ruff.exe"
FIXTURES = REPO / "src" / "lint" / "python" / "fixtures"
REGISTRY = REPO / "src" / "lint" / "python" / "tests" / "fixtures.rs"
RULES = REPO / "src" / "rules" / "families" / "python.rs"
FIXABLE = pathlib.Path(__file__).resolve().parent / "fixable_kerbo.txt"


def selected_codes():
    return sorted(set(re.findall(r"\b([A-Z]+[0-9]+) =>", RULES.read_text(encoding="utf-8"))))


def fixable_codes():
    return [line.strip() for line in FIXABLE.read_text(encoding="utf-8").split() if line.strip()]


def registered_fixtures():
    text = REGISTRY.read_text(encoding="utf-8")
    return re.findall(r'\(\s*"([^"]+)",\s*include_str!\("\.\./fixtures/([^"]+)"\)', text)


def golden_stem(logical):
    flat = logical.replace("/__init__", "_init").replace("/", "_")
    return flat.rsplit(".", 1)[0]


def ruff_fixed(logical, source, unsafe):
    with tempfile.TemporaryDirectory() as directory:
        target = pathlib.Path(directory) / logical
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_bytes(source)
        command = [
            str(RUFF), "check", "--isolated", "--no-cache", "--exit-zero", "--fix",
            "--select", ",".join(selected_codes()), "--fixable", ",".join(fixable_codes()),
            str(target),
        ]
        if unsafe:
            command.append("--unsafe-fixes")
        subprocess.run(command, capture_output=True, cwd=directory, check=True)
        return target.read_bytes()


def main():
    check_only = "--check" in sys.argv
    changed = []
    for logical, file in registered_fixtures():
        source = (FIXTURES / file).read_bytes()
        for suffix, unsafe in (("fixed", False), ("unsafe", True)):
            golden = FIXTURES / f"{golden_stem(logical)}.{suffix}"
            fixed = ruff_fixed(logical, source, unsafe)
            current = golden.read_bytes() if golden.exists() else source
            if fixed != current:
                changed.append(golden.name)
                if not check_only:
                    if fixed == source:
                        golden.unlink()
                    else:
                        golden.write_bytes(fixed)
    print("\n".join(changed) if changed else "sin cambios")


if __name__ == "__main__":
    main()
