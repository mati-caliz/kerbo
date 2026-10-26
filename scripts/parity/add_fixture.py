"""Registra un fixture de Python con su salida de ruff al lado (`.ruff`).

Uso: python -I scripts/parity/add_fixture.py <archivo.py en fixtures/> [ruta lógica]
La ruta lógica es la que ve kerbo (por ejemplo `package/__init__.py`); por defecto, el nombre.
Después hay que correr ruff_fixed.py para los `.fixed` y `.unsafe`.
"""
import json
import pathlib
import re
import subprocess
import sys
import tempfile

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
from ruff_fixed import FIXTURES, REGISTRY, RUFF, golden_stem, selected_codes


def byte_column(line, character_column):
    return len(line[: character_column - 1].encode("utf-8")) + 1


def ruff_findings(logical, source):
    with tempfile.TemporaryDirectory() as directory:
        target = pathlib.Path(directory) / logical
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_bytes(source)
        command = [
            str(RUFF), "check", "--isolated", "--no-cache", "--exit-zero", "--output-format", "json",
            "--select", ",".join(selected_codes()), str(target),
        ]
        output = subprocess.run(command, capture_output=True, cwd=directory, check=True).stdout
    lines = source.decode("utf-8").splitlines()
    rendered = []
    for finding in json.loads(output):
        if not finding.get("code"):
            continue
        row = finding["location"]["row"]
        column = finding["location"]["column"]
        text = lines[row - 1] if row <= len(lines) else ""
        rendered.append(f"{row}:{byte_column(text, column)} {finding['code']}")
    return rendered


def register(logical, file, golden):
    text = REGISTRY.read_text(encoding="utf-8")
    count = int(re.search(r"FIXTURES: \[\(&str, &str, &str\); (\d+)\]", text).group(1))
    text = text.replace(f"&str); {count}]", f"&str); {count + 1}]", 1)
    entry = (
        f'    (\n        "{logical}",\n        include_str!("../fixtures/{file}"),\n'
        f'        include_str!("../fixtures/{golden}"),\n    ),\n];\n'
    )
    text = text[: text.rindex("];")] + entry.removesuffix("];\n") + "];\n"
    REGISTRY.write_text(text, encoding="utf-8", newline="\n")


def main():
    file = sys.argv[1]
    logical = sys.argv[2] if len(sys.argv) > 2 else file
    golden = f"{golden_stem(logical)}.ruff"
    findings = ruff_findings(logical, (FIXTURES / file).read_bytes())
    (FIXTURES / golden).write_text("".join(f"{line}\n" for line in findings), encoding="utf-8", newline="\n")
    if f'"{logical}"' not in REGISTRY.read_text(encoding="utf-8"):
        register(logical, file, golden)
    print(f"{golden}: {len(findings)} avisos")


if __name__ == "__main__":
    main()
