"""Busca las líneas de un fixture por las que ruff revierte todos los arreglos del archivo.

Uso: python -I scripts/parity/reverting_lines.py <fixture.py> [--unsafe]
Ruff aplica los arreglos, ve que el resultado tiene un error de sintaxis y deja el archivo como
estaba ("Fix introduced a syntax error. Reverting all changes."). Prueba sacando una línea por vez
y lista las que, al sacarlas, hacen que ruff deje de revertir.
"""
import ast
import pathlib
import subprocess
import sys
import tempfile

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
from ruff_fixed import RUFF, fixable_codes, selected_codes

REVERTING = "Reverting all changes"


def reverts(source, unsafe):
    with tempfile.TemporaryDirectory() as directory:
        target = pathlib.Path(directory) / "probe.py"
        target.write_text(source, encoding="utf-8", newline="")
        command = [
            str(RUFF), "check", "--isolated", "--no-cache", "--exit-zero", "--fix",
            "--select", ",".join(selected_codes()), "--fixable", ",".join(fixable_codes()),
            str(target),
        ]
        if unsafe:
            command.append("--unsafe-fixes")
        result = subprocess.run(command, capture_output=True, text=True, cwd=directory, check=False)
        return REVERTING in result.stdout + result.stderr


def parses(source):
    try:
        ast.parse(source)
    except SyntaxError:
        return False
    return True


def main():
    path = pathlib.Path(sys.argv[1])
    unsafe = "--unsafe" in sys.argv
    lines = path.read_text(encoding="utf-8").splitlines(keepends=True)
    if not reverts("".join(lines), unsafe):
        print("ruff no revierte este archivo")
        return
    for index, line in enumerate(lines):
        without = "".join(lines[:index] + lines[index + 1:])
        if line.strip() and parses(without) and not reverts(without, unsafe):
            print(f"{index + 1}: {line.rstrip()}")


if __name__ == "__main__":
    main()
