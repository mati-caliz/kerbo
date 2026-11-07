#!/usr/bin/env python
"""Las versiones de referencia de la paridad, de scripts/parity/versions.toml.

    python scripts/parity/versions.py version shellcheck     # 0.11.0
    python scripts/parity/versions.py image shellcheck       # koalaman/shellcheck-alpine:v0.11.0
    python scripts/parity/versions.py check                  # Cargo.toml, gramáticas, ESLint y la documentación dicen lo mismo
    python scripts/parity/versions.py outdated               # cuánto atraso hay contra lo último publicado
"""

import json
import os
import pathlib
import re
import sys
import tomllib
import urllib.request

HERE = pathlib.Path(__file__).resolve().parent
REPO = HERE.parent.parent
VERSIONS = HERE / "versions.toml"
CARGO = REPO / "Cargo.toml"
DOCS = REPO / "docs"
ESLINT_PACKAGE = pathlib.Path(os.environ.get("KERBO_PARITY", REPO / ".parity")) / "eslint" / "package.json"
USER_AGENT = "kerbo-parity-versions"
TIMEOUT_SECONDS = 20
NUMBER = re.compile(r"\d+")
VERSION_IN_TAG = re.compile(r"\d+(?:\.\d+)*(?:-[0-9A-Za-z.]+)?")
LATEST_TAG = "latest"


def load():
    return tomllib.loads(VERSIONS.read_text(encoding="utf-8"))


def entry(name):
    data = load()
    for table in ("tools", "grammars"):
        if name in data.get(table, {}):
            return data[table][name]
    sys.exit(f"versions.py: {name} no está en versions.toml")


def expand(template, version):
    parts = version.split("-")[0].split(".")
    return template.format(version=version, minor=".".join(parts[:2]), major=parts[0])


def fetch_json(url):
    request = urllib.request.Request(url, headers={"User-Agent": USER_AGENT, "Accept": "application/json"})
    token = os.environ.get("GITHUB_TOKEN")
    if token and "api.github.com" in url:
        request.add_header("Authorization", f"Bearer {token}")
    with urllib.request.urlopen(request, timeout=TIMEOUT_SECONDS) as response:
        return json.load(response)


def latest_version(source):
    kind, _, name = source.partition(":")
    if kind == "pypi":
        return fetch_json(f"https://pypi.org/pypi/{name}/json")["info"]["version"]
    if kind == "npm":
        return fetch_json(f"https://registry.npmjs.org/{name}/latest")["version"]
    if kind == "crates":
        return fetch_json(f"https://crates.io/api/v1/crates/{name}")["crate"]["max_stable_version"]
    if kind == "github":
        tag = fetch_json(f"https://api.github.com/repos/{name}/releases/latest")["tag_name"]
        return VERSION_IN_TAG.search(tag).group(0)
    if kind == "go":
        return fetch_json("https://go.dev/dl/?mode=json")[0]["version"].removeprefix("go")
    if kind == "dockerhub":
        tags = fetch_json(f"https://hub.docker.com/v2/repositories/{name}/tags?page_size=20&ordering=last_updated")
        return next(tag["name"] for tag in tags["results"] if tag["name"] != LATEST_TAG)
    raise ValueError(f"fuente desconocida: {source}")


def numbers(version):
    return [int(part) for part in NUMBER.findall(version.split("-")[0])]


def distance(pinned, latest):
    old, new = numbers(pinned), numbers(latest)
    width = max(len(old), len(new))
    old += [0] * (width - len(old))
    new += [0] * (width - len(new))
    if new <= old:
        return "al día" if pinned == latest or "-" not in pinned else "salió la estable"
    if new[0] != old[0]:
        return "mayor"
    if len(new) > 1 and new[1] != old[1]:
        return "menor"
    return "parche"


def outdated():
    sys.stdout.reconfigure(encoding="utf-8")
    data = load()
    rows = []
    for table in ("tools", "grammars"):
        for name, tool in data.get(table, {}).items():
            family = tool.get("family", "gramáticas")
            try:
                latest = latest_version(tool["latest"])
                floats = tool.get("floating") and latest.startswith(f"{tool['version']}.")
                state = "al día (la imagen sigue el último parche)" if floats else distance(tool["version"], latest)
            except Exception as problem:  # noqa: BLE001 - una fuente caída no frena el resto
                latest, state = "?", f"no se pudo averiguar ({type(problem).__name__})"
            rows.append((family, name, tool["version"], latest, state))
    width = max(len(row[1]) for row in rows)
    for family, name, pinned, latest, state in sorted(rows):
        print(f"{family:<13} {name:<{width}} {pinned:<12} {latest:<12} {state}")
    behind = [row for row in rows if row[4] in ("parche", "menor", "mayor")]
    print(f"\n{len(behind)} de {len(rows)} atrasadas")


def cargo_versions():
    text = CARGO.read_text(encoding="utf-8")
    found = {}
    for line in text.splitlines():
        match = re.match(r'^(tree-sitter[\w-]*)\s*=\s*(?:"([^"]+)"|\{.*?version\s*=\s*"([^"]+)")', line)
        if match:
            found[match.group(1)] = match.group(2) or match.group(3)
    return found


def check():
    sys.stdout.reconfigure(encoding="utf-8")
    data = load()
    problems = []
    cargo = cargo_versions()
    for name, grammar in data.get("grammars", {}).items():
        if cargo.get(name) != grammar["version"]:
            problems.append(f"Cargo.toml dice {name} {cargo.get(name)}, versions.toml {grammar['version']}")
        copied = grammar.get("copied")
        if copied:
            copied_cargo = (REPO / copied / "Cargo.toml").read_text(encoding="utf-8")
            if f'version = "{grammar["version"]}"' not in copied_cargo:
                problems.append(f"{copied}/Cargo.toml no dice la versión {grammar['version']}")
    documentation = "\n".join(page.read_text(encoding="utf-8") for page in sorted(DOCS.glob("*.md")))
    for name, tool in data["tools"].items():
        cited = tool.get("cited_as")
        if cited and expand(cited, tool["version"]) not in documentation:
            problems.append(f"docs/ no cita «{expand(cited, tool['version'])}» ({name})")
    if ESLINT_PACKAGE.exists():
        dependencies = json.loads(ESLINT_PACKAGE.read_text(encoding="utf-8")).get("dependencies", {})
        for name, tool in data["tools"].items():
            if tool.get("family") != "ecmascript":
                continue
            package = tool.get("package", name)
            written = dependencies.get(package, "").lstrip("^~")
            if written and not tool["version"].startswith(written):
                problems.append(f".parity/eslint/package.json dice {package} {written}, versions.toml {tool['version']}")
    for problem in problems:
        print(problem)
    if problems:
        sys.exit(1)
    print("versions.toml, Cargo.toml, las gramáticas, ESLint y la documentación coinciden")


def main():
    if len(sys.argv) < 2:
        sys.exit(__doc__)
    command = sys.argv[1]
    if command == "version":
        print(entry(sys.argv[2])["version"])
    elif command == "image":
        tool = entry(sys.argv[2])
        print(expand(tool["image"], tool["version"]))
    elif command == "check":
        check()
    elif command == "outdated":
        outdated()
    else:
        sys.exit(__doc__)


if __name__ == "__main__":
    main()
