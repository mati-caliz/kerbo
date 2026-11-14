#!/usr/bin/env python
"""Corre PMD con las reglas del quickstart que kerbo imita sobre un repo del corpus y lo compara con
kerbo, por regla, archivo y línea. PMD avisa una vez por declaración con todos los modificadores de
más y kerbo uno por modificador: de los dos lados cuenta una vez cada regla en cada línea. Lo que ignora configs/<repo>.kerbo.toml tampoco se cuenta de PMD.

    python scripts/parity/java/pmd.py junit4
    python scripts/parity/java/pmd.py guava --skip-pmd
    python scripts/parity/java/pmd.py --golden src/lint/fixtures/Pmd.java

Con --golden escribe al lado del fixture un .pmd con una línea `<línea> <regla>` por aviso.

Necesita la distribución de la versión de versions.toml en .parity/pmd/ (se baja sola), java en el
host y un .parity/qodana/<repo>/kerbo-pmd.json sacado con `JAVA_REFERENCE=pmd kerbo.sh`.
"""

import collections
import fnmatch
import json
import os
import pathlib
import subprocess
import sys
import urllib.request
import zipfile

from checkstyle import ignored_patterns

ROOT = pathlib.Path(__file__).resolve().parents[3]
PARITY = pathlib.Path(os.environ.get("KERBO_PARITY", ROOT / ".parity"))
VERSIONS = ROOT / "scripts" / "parity" / "versions.py"
ZIP_URL = "https://github.com/pmd/pmd/releases/download/pmd_releases%2F{version}/pmd-dist-{version}-bin.zip"
RULES = {
    "category/java/codestyle.xml/UnnecessaryModifier": "redundant-modifier",
    "category/java/design.xml/ClassWithOnlyPrivateConstructorsShouldBeFinal": "final-class",
}
SHOWN = int(os.environ.get("SHOWN", "15"))


def pmd_version():
    return subprocess.run([sys.executable, str(VERSIONS), "version", "pmd"], capture_output=True, text=True, check=True).stdout.strip()


def ensure_pmd():
    folder = PARITY / "pmd"
    folder.mkdir(parents=True, exist_ok=True)
    version = pmd_version()
    launcher = folder / f"pmd-bin-{version}" / "bin" / ("pmd.bat" if os.name == "nt" else "pmd")
    if not launcher.exists():
        archive = folder / f"pmd-{version}.zip"
        urllib.request.urlretrieve(ZIP_URL.format(version=version), archive)
        with zipfile.ZipFile(archive) as bundle:
            bundle.extractall(folder)
        archive.unlink()
    return folder, launcher


def run_pmd(repo, folder, launcher):
    output = folder / f"{repo}.json"
    subprocess.run(
        [str(launcher), "check", "--no-cache", "--no-progress", "-R", ",".join(RULES), "-f", "json", "-r", str(output),
         "-d", str(PARITY / "corpus" / repo)],
        capture_output=True, check=False,
    )
    return output


def pmd_findings(output, repo):
    corpus = (PARITY / "corpus" / repo).resolve()
    ignored = ignored_patterns(repo)
    names = {rule.rsplit("/", 1)[-1]: kebab for rule, kebab in RULES.items()}
    found = collections.defaultdict(collections.Counter)
    for file in json.loads(output.read_text(encoding="utf-8"))["files"]:
        path = pathlib.Path(file["filename"]).resolve().relative_to(corpus).as_posix()
        if any(fnmatch.fnmatch(path, pattern) for pattern in ignored):
            continue
        for violation in file["violations"]:
            found[names[violation["rule"]]][(path, violation["beginline"])] = 1
    return found


def kerbo_findings(repo):
    found = collections.defaultdict(collections.Counter)
    for item in json.loads((PARITY / "qodana" / repo / "kerbo-pmd.json").read_text(encoding="utf-8")):
        if item["rule"] in RULES.values():
            found[item["rule"]][(item["path"].removeprefix("./"), item["line"])] = 1
    return found


def write_golden(fixture, launcher):
    output = fixture.with_suffix(".json")
    subprocess.run(
        [str(launcher), "check", "--no-cache", "--no-progress", "-R", ",".join(RULES), "-f", "json", "-r", str(output),
         "-d", str(fixture)],
        capture_output=True, check=False,
    )
    names = {rule.rsplit("/", 1)[-1]: kebab for rule, kebab in RULES.items()}
    report = json.loads(output.read_text(encoding="utf-8"))
    output.unlink()
    found = sorted({(violation["beginline"], names[violation["rule"]]) for file in report["files"] for violation in file["violations"]})
    fixture.with_suffix(".pmd").write_text("".join(f"{line} {rule}\n" for line, rule in found), encoding="utf-8")


def main():
    sys.stdout.reconfigure(encoding="utf-8")
    folder, launcher = ensure_pmd()
    if sys.argv[1] == "--golden":
        write_golden(pathlib.Path(sys.argv[2]), launcher)
        return
    repo = sys.argv[1]
    output = folder / f"{repo}.json" if "--skip-pmd" in sys.argv else run_pmd(repo, folder, launcher)
    reference = pmd_findings(output, repo)
    kerbo = kerbo_findings(repo)
    totals = collections.Counter()
    for rule in sorted(RULES.values()):
        missing = reference[rule] - kerbo[rule]
        extra = kerbo[rule] - reference[rule]
        total = sum(reference[rule].values())
        totals.update(coinciden=total - sum(missing.values()), faltan=sum(missing.values()), sobran=sum(extra.values()))
        status = "igual" if not missing and not extra else f"faltan {sum(missing.values())}, sobran {sum(extra.values())}"
        print(f"{rule}: PMD {total}, {status}")
        for title, diff in (("faltan", missing), ("sobran", extra)):
            for (path, line), count in sorted(diff.items())[:SHOWN]:
                print(f"    {title} {path}:{line} x{count}")
    print(f"== {repo}: {totals['coinciden']} coinciden, {totals['faltan']} faltan, {totals['sobran']} de más")


if __name__ == "__main__":
    main()
