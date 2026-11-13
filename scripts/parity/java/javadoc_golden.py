#!/usr/bin/env python
"""Saca de Checkstyle el golden de un fixture de Javadoc: una línea `<línea> <regla>` por aviso de las
reglas de Javadoc de sun_checks.xml, juntando los que caen en la misma posición como checkstyle.py.

    python scripts/parity/java/javadoc_golden.py src/lint/fixtures/Javadoc.java

Escribe el .javadoc al lado del fixture. Usa el jar de .parity/checkstyle (lo baja checkstyle.py).
"""

import os
import pathlib
import re
import subprocess
import sys
import tempfile

ROOT = pathlib.Path(__file__).resolve().parents[3]
PARITY = pathlib.Path(os.environ.get("KERBO_PARITY", ROOT / ".parity"))
CHECKS = ["InvalidJavadocPosition", "JavadocMethod", "JavadocType", "JavadocVariable", "MissingJavadocMethod"]
CONFIGURATION = """<?xml version="1.0"?>
<!DOCTYPE module PUBLIC "-//Checkstyle//DTD Checkstyle Configuration 1.3//EN" "https://checkstyle.org/dtds/configuration_1_3.dtd">
<module name="Checker">
  <module name="TreeWalker">
{modules}
  </module>
</module>
"""
VIOLATION = re.compile(r"^\[\w+\] .*?:(\d+):(?:(\d+):)? .* \[(\w+)\]$")


def kebab(name):
    return re.sub(r"(?<!^)(?=[A-Z])", "-", name).lower()


def main():
    fixture = pathlib.Path(sys.argv[1])
    jar = next((PARITY / "checkstyle").glob("checkstyle-*-all.jar"))
    modules = "\n".join(f'    <module name="{check}"/>' for check in CHECKS)
    with tempfile.TemporaryDirectory() as folder:
        configuration = pathlib.Path(folder) / "javadoc.xml"
        configuration.write_text(CONFIGURATION.format(modules=modules), encoding="utf-8")
        result = subprocess.run(["java", "-jar", str(jar), "-c", str(configuration), str(fixture)], capture_output=True, text=True, check=False)
    seen = set()
    lines = []
    for output in result.stdout.splitlines():
        match = VIOLATION.match(output.strip())
        if match and match.groups() not in seen:
            seen.add(match.groups())
            lines.append(f"{match.group(1)} {kebab(match.group(3))}")
    golden = fixture.with_suffix(".javadoc")
    golden.write_text("\n".join(sorted(lines, key=lambda line: (int(line.split()[0]), line))) + "\n", encoding="utf-8", newline="\n")
    print(f"{golden}: {len(lines)} avisos")


if __name__ == "__main__":
    main()
