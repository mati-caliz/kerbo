"""Estándar de calidad: lo copia quality/sync.sh y no se edita en el repo.

Revisa que los identificadores estén en inglés: los saca del código (TypeScript con su parser,
Python con ast, Go y Rust sin strings ni comentarios), los parte en palabras y se los pasa a cspell
con el diccionario del estándar. Falla ante una palabra que cspell no conoce y que no está en
.identifiers-baseline.txt, la lista de lo que ya había al adoptarlo, que sólo se achica.

    python3 scripts/check_identifiers.py            chequea
    python3 scripts/check_identifiers.py --update   reescribe la línea base sin sumar palabras
"""

from __future__ import annotations

import io
import re
import shutil
import subprocess
import sys
import tokenize
from collections import defaultdict
from pathlib import Path

CSPELL_VERSION = "10.4.1"
BASELINE_NAME = ".identifiers-baseline.txt"
VENDORED_FIELD_COUNT = 2
CONFIG_NAME = ".cspell.json"
MANIFEST_NAME = ".quality"
MINIMUM_WORD_LENGTH = 4
EXAMPLES_PER_WORD = 3
TYPESCRIPT_SUFFIXES = {".ts", ".tsx", ".mts", ".cts"}
CODE_SUFFIXES = TYPESCRIPT_SUFFIXES | {".py", ".go", ".rs"}
SKIPPED_PARTS = {
    "node_modules",
    "dist",
    "build",
    "out",
    ".next",
    ".astro",
    "vendor",
    "fixtures",
    "migrations",
}
SKIPPED_SUFFIXES = (".gen.go", ".d.ts")

GO_RUST_LITERALS = re.compile(
    r"//[^\n]*"
    r"|/\*.*?\*/"
    r"|(?<![A-Za-z0-9_])b?r(#*)\".*?\"\1"
    r"|`[^`]*`"
    r"|b?\"(?:\\.|[^\"\\])*\""
    r"|b?'(?:\\u\{[0-9a-fA-F]+\}|\\.|[^'\\])'",
    re.DOTALL,
)
IDENTIFIER = re.compile(r"(?<![A-Za-z0-9_])[A-Za-z_][A-Za-z0-9_]*")
WORD_BOUNDARIES = (re.compile(r"([a-z0-9])([A-Z])"), re.compile(r"([A-Z]+)([A-Z][a-z])"))


def executable(name: str) -> str:
    path = shutil.which(name)
    if path is None:
        sys.exit(f"falta {name} en el PATH")
    return path


def repo_files(repo: Path, vendored: list[str]) -> list[Path]:
    listed = subprocess.run(
        [
            executable("git"),
            "-C",
            str(repo),
            "ls-files",
            "--cached",
            "--others",
            "--exclude-standard",
        ],
        capture_output=True,
        text=True,
        check=True,
    ).stdout.splitlines()
    files = []
    for name in listed:
        path = Path(name)
        if path.suffix not in CODE_SUFFIXES or name.endswith(SKIPPED_SUFFIXES):
            continue
        if SKIPPED_PARTS.intersection(path.parts) or any(
            name.startswith(prefix) for prefix in vendored
        ):
            continue
        if (repo / path).is_file():
            files.append(path)
    return files


def vendored_prefixes(repo: Path) -> list[str]:
    manifest = repo / MANIFEST_NAME
    prefixes = []
    for line in manifest.read_text(encoding="utf-8").splitlines():
        fields = line.split()
        if len(fields) >= VENDORED_FIELD_COUNT and fields[0] == "vendored":
            prefixes.append(fields[1].rstrip("/") + "/")
    return prefixes


def python_identifiers(source: str) -> set[str]:
    """Los nombres según tokenize: no entra en strings ni comentarios y tolera las plantillas."""
    tokens = tokenize.generate_tokens(io.StringIO(source).readline)
    return {token.string for token in tokens if token.type == tokenize.NAME}


def go_rust_identifiers(source: str) -> set[str]:
    return set(IDENTIFIER.findall(GO_RUST_LITERALS.sub(" ", source)))


def typescript_package(repo: Path, file: Path) -> Path | None:
    """El paquete más cercano con typescript instalado, o si no cualquiera del repo.

    Un preview o un caso de prueba suelto no tiene node_modules propio.
    """
    for directory in (repo / file).parents:
        if (directory / "node_modules" / "typescript").is_dir():
            return directory
        if directory == repo:
            break
    installed = next(repo.glob("*/node_modules/typescript"), None) or next(
        repo.glob("*/*/node_modules/typescript"), None
    )
    return None if installed is None else installed.parent.parent


def declares_typescript(repo: Path) -> bool:
    return any(
        '"typescript"' in manifest.read_text(encoding="utf-8")
        for manifest in repo.glob("**/package.json")
        if "node_modules" not in manifest.parts
    )


def typescript_identifiers(repo: Path, files: list[Path]) -> dict[Path, set[str]]:
    """Los identificadores de cada archivo TypeScript, por paquete.

    Un repo que no declara typescript en ningún package.json no tiene código TypeScript propio: sus
    .ts son material de prueba de otra herramienta y no se revisan.
    """
    if files and not declares_typescript(repo):
        return {}
    by_package: dict[Path, list[Path]] = defaultdict(list)
    for file in files:
        package = typescript_package(repo, file)
        if package is None:
            sys.exit(f"{file}: no hay typescript instalado en el repo; corré pnpm install")
        by_package[package].append(file)
    script = Path(__file__).with_name("identifiers.mjs")
    names: dict[Path, set[str]] = defaultdict(set)
    for package, package_files in by_package.items():
        output = subprocess.run(
            [executable("node"), str(script), str(package)],
            input="\n".join(str(repo / file) for file in package_files),
            capture_output=True,
            text=True,
            check=True,
        ).stdout
        for line in output.splitlines():
            file_name, _, identifier = line.partition("\t")
            names[Path(file_name).relative_to(repo)].add(identifier)
    return names


def words_of(identifier: str) -> list[str]:
    spaced = identifier
    for boundary in WORD_BOUNDARIES:
        spaced = boundary.sub(r"\1 \2", spaced)
    return [
        word.lower() for word in re.split(r"[^A-Za-z]+", spaced) if len(word) >= MINIMUM_WORD_LENGTH
    ]


def identifiers_by_file(repo: Path, files: list[Path]) -> dict[Path, set[str]]:
    found = typescript_identifiers(
        repo, [file for file in files if file.suffix in TYPESCRIPT_SUFFIXES]
    )
    for file in files:
        if file.suffix in TYPESCRIPT_SUFFIXES:
            continue
        source = (repo / file).read_text(encoding="utf-8", errors="replace")
        found[file] = (
            python_identifiers(source) if file.suffix == ".py" else go_rust_identifiers(source)
        )
    return found


def unknown_words(repo: Path, words: set[str]) -> set[str]:
    result = subprocess.run(
        [
            executable("pnpm"),
            "dlx",
            f"cspell@{CSPELL_VERSION}",
            "--no-progress",
            "--no-summary",
            "--words-only",
            "--unique",
            "--config",
            str(repo / CONFIG_NAME),
            "stdin",
        ],
        input="\n".join(sorted(words)) + "\n",
        capture_output=True,
        text=True,
        check=False,
        cwd=repo,
    )
    if result.returncode not in (0, 1):
        sys.exit(f"cspell falló: {result.stderr.strip()}")
    return {word.lower() for word in result.stdout.split()}


def committed_baseline(repo: Path) -> set[str] | None:
    shown = subprocess.run(
        [executable("git"), "-C", str(repo), "show", f"HEAD:{BASELINE_NAME}"],
        capture_output=True,
        text=True,
        check=False,
    )
    return set(shown.stdout.split()) if shown.returncode == 0 else None


def main() -> int:
    repo = Path(__file__).resolve().parent.parent
    update = "--update" in sys.argv[1:]
    files = repo_files(repo, vendored_prefixes(repo))
    examples: dict[str, list[str]] = defaultdict(list)
    for file, identifiers in identifiers_by_file(repo, files).items():
        for identifier in sorted(identifiers):
            for word in words_of(identifier):
                if len(examples[word]) < EXAMPLES_PER_WORD:
                    examples[word].append(f"{identifier} ({file})")
    unknown = unknown_words(repo, set(examples))
    baseline_file = repo / BASELINE_NAME
    previous = committed_baseline(repo)
    if update:
        kept = unknown if previous is None else unknown & previous
        baseline_file.write_text("".join(f"{word}\n" for word in sorted(kept)), encoding="utf-8")
        return 0
    baseline = (
        set(baseline_file.read_text(encoding="utf-8").split()) if baseline_file.exists() else set()
    )
    if previous is not None and baseline - previous:
        sys.stderr.write(f"{BASELINE_NAME} sumó palabras: la línea base sólo se achica\n")
        return 1
    new_words = sorted(unknown - baseline)
    for word in new_words:
        sys.stderr.write(f"{word}: {', '.join(examples[word])}\n")
    if new_words:
        sys.stderr.write(
            "Identificadores que no están en inglés: renombralos, o sumá la palabra de dominio al "
            "diccionario del estándar (quality/identifiers/cspell.standard.json).\n"
        )
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
