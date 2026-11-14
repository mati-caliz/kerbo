#!/usr/bin/env python
"""Corre Checkstyle con sun_checks.xml sobre un repo del corpus y lo compara con kerbo, por regla,
archivo y línea (las columnas de Checkstyle expanden los tabs a 8, las de kerbo cuentan bytes).
Checkstyle avisa dos veces el mismo token cuando falta el blanco de los dos lados; kerbo, una: de
los dos lados se cuenta una vez cada regla en cada posición. Lo que ignora configs/<repo>.kerbo.toml
tampoco se cuenta de Checkstyle.

    python scripts/parity/java/checkstyle.py commons-lang
    python scripts/parity/java/checkstyle.py junit4 --skip-checkstyle

Necesita el .jar `all` de la versión de versions.toml en .parity/checkstyle/ (se baja solo) y un
.parity/qodana/<repo>/kerbo.json sacado con kerbo.sh.
"""

import collections
import fnmatch
import json
import os
import pathlib
import re
import subprocess
import sys
import tomllib
import urllib.request
import xml.etree.ElementTree as ElementTree

ROOT = pathlib.Path(__file__).resolve().parents[3]
PARITY = pathlib.Path(os.environ.get("KERBO_PARITY", ROOT / ".parity"))
VERSIONS = ROOT / "scripts" / "parity" / "versions.py"
JAR_URL = "https://github.com/checkstyle/checkstyle/releases/download/checkstyle-{version}/checkstyle-{version}-all.jar"
NAME_EXCEPTIONS = {"AvoidStarImport": "star-import", "FinalParameters": "final-parameter", "UnusedImports": "unused-import", "RegexpSingleline": "trailing-spaces"}
NOT_MEASURED = {"LineLength", "Indentation", "MagicNumber", "FileLength", "MethodLength", "ParameterNumber"}
SHOWN = 15


def kebab(name):
    return NAME_EXCEPTIONS.get(name) or re.sub(r"(?<!^)(?=[A-Z])", "-", name).lower()


def checkstyle_version():
    return subprocess.run([sys.executable, str(VERSIONS), "version", "checkstyle"], capture_output=True, text=True, check=True).stdout.strip()



def ensure_jar():
    folder = PARITY / "checkstyle"
    folder.mkdir(parents=True, exist_ok=True)
    version = checkstyle_version()
    jar = folder / f"checkstyle-{version}-all.jar"
    if not jar.exists():
        urllib.request.urlretrieve(JAR_URL.format(version=version), jar)
    return folder, jar


def run_checkstyle(repo, folder, jar):
    configuration = folder / "sun_checks.xml"
    if not configuration.exists():
        subprocess.run(["jar", "xf", str(jar.resolve()), "sun_checks.xml"], cwd=folder, check=True)
    output = folder / f"{repo}.xml"
    subprocess.run(
        ["java", "-jar", str(jar), "-c", str(configuration), "-f", "xml", "-o", str(output), str(PARITY / "corpus" / repo)],
        capture_output=True, check=False,
    )
    return output


def ignored_patterns(repo):
    configuration = ROOT / "scripts" / "parity" / "java" / "configs" / f"{repo}.kerbo.toml"
    if not configuration.exists():
        return []
    ignores = tomllib.loads(configuration.read_text(encoding="utf-8")).get("ignore", [])
    return [pattern for ignore in ignores for pattern in ignore["paths"]]


def checkstyle_findings(output, repo):
    corpus = (PARITY / "corpus" / repo).resolve()
    ignored = ignored_patterns(repo)
    seen = set()
    found = collections.defaultdict(collections.Counter)
    for file in ElementTree.parse(output).getroot():
        path = pathlib.Path(file.get("name")).resolve().relative_to(corpus).as_posix()
        for error in file:
            name = error.get("source").rsplit(".", 1)[-1].removesuffix("Check")
            position = (kebab(name), path, int(error.get("line")), error.get("column"))
            measured = name not in NOT_MEASURED and path.endswith(".java")
            if measured and position not in seen and not any(fnmatch.fnmatch(path, pattern) for pattern in ignored):
                seen.add(position)
                found[position[0]][position[1:3]] += 1
    return found


def kerbo_findings(repo, rules):
    found = collections.defaultdict(collections.Counter)
    seen = set()
    for item in json.loads((PARITY / "qodana" / repo / "kerbo.json").read_text(encoding="utf-8")):
        position = (item["rule"], item["path"].removeprefix("./"), item["line"], item["column"])
        if item["rule"] in rules and position not in seen:
            seen.add(position)
            found[position[0]][position[1:3]] += 1
    return found


def main():
    repo = sys.argv[1]
    folder, jar = ensure_jar()
    output = folder / f"{repo}.xml" if "--skip-checkstyle" in sys.argv else run_checkstyle(repo, folder, jar)
    reference = checkstyle_findings(output, repo)
    kerbo = kerbo_findings(repo, set(reference))
    totals = collections.Counter()
    for rule in sorted(reference):
        missing = reference[rule] - kerbo[rule]
        extra = kerbo[rule] - reference[rule]
        total = sum(reference[rule].values())
        totals.update(matched=total - sum(missing.values()), missing_count=sum(missing.values()), extra_count=sum(extra.values()))
        status = "igual" if not missing and not extra else f"faltan {sum(missing.values())}, sobran {sum(extra.values())}"
        print(f"{rule}: Checkstyle {total}, {status}")
        for title, diff in (("faltan", missing), ("sobran", extra)):
            for (path, line), count in sorted(diff.items())[:SHOWN]:
                print(f"    {title} {path}:{line} x{count}")
    print(f"== {repo}: {totals['matched']} coinciden, {totals['missing_count']} faltan, {totals['extra_count']} de más")


if __name__ == "__main__":
    main()
