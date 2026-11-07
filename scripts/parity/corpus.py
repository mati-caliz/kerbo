#!/usr/bin/env python
"""El corpus de paridad (.parity/corpus) sale de corpus.toml: cada repo con su URL y el commit con
que se midió. Así .parity es una caché que se puede rearmar entera.

    python scripts/parity/corpus.py sync      # clona lo que falta, en su commit
    python scripts/parity/corpus.py record    # anota en corpus.toml el commit que tiene cada repo
    python scripts/parity/corpus.py status    # qué falta, qué cambió y qué está de más
"""

import os
import pathlib
import subprocess
import sys
import tomllib

HERE = pathlib.Path(__file__).resolve().parent
REPO = HERE.parent.parent
MANIFEST = HERE / "corpus.toml"
CORPUS = pathlib.Path(os.environ.get("KERBO_PARITY", REPO / ".parity")) / "corpus"
GIT = ["git", "-c", "core.longpaths=true", "-c", "core.autocrlf=false"]


def manifest():
    return tomllib.loads(MANIFEST.read_text(encoding="utf-8"))["repos"]


def git(*arguments, cwd=None):
    return subprocess.run([*GIT, *arguments], cwd=cwd, capture_output=True, text=True, check=True).stdout.strip()


def head(folder):
    if not (folder / ".git").exists():
        return None
    try:
        return git("rev-parse", "HEAD", cwd=folder)
    except subprocess.CalledProcessError:
        return None


def clone(name, repo):
    target = CORPUS / name
    target.mkdir(parents=True)
    git("init", "-q", cwd=target)
    git("remote", "add", "origin", repo["url"], cwd=target)
    git("fetch", "-q", "--depth", "1", "origin", repo.get("commit") or "HEAD", cwd=target)
    git("checkout", "-q", "FETCH_HEAD", cwd=target)
    print(f"{name}: clonado en {head(target)[:12]}")


def sync():
    CORPUS.mkdir(parents=True, exist_ok=True)
    for name, repo in manifest().items():
        if (CORPUS / name).exists():
            continue
        try:
            clone(name, repo)
        except subprocess.CalledProcessError as problem:
            print(f"{name}: no se pudo clonar ({problem.stderr.strip()})")


def record():
    text = MANIFEST.read_text(encoding="utf-8")
    for name, repo in manifest().items():
        commit = head(CORPUS / name)
        if commit and commit != repo.get("commit"):
            old = f'commit = "{repo.get("commit", "")}"'
            block = text.index(f"[repos.{name}]")
            position = text.index(old, block)
            text = text[:position] + f'commit = "{commit}"' + text[position + len(old):]
            print(f"{name}: {commit[:12]}")
    MANIFEST.write_text(text, encoding="utf-8", newline="")


def status():
    repos = manifest()
    present = {path.name for path in CORPUS.iterdir() if path.is_dir()} if CORPUS.exists() else set()
    for name, repo in repos.items():
        commit = head(CORPUS / name) if name in present else None
        if name not in present:
            state = "falta"
        elif commit != repo.get("commit"):
            state = f"en {commit[:12] if commit else 'sin git'}, el manifiesto dice {repo.get('commit', '')[:12]}"
        else:
            state = "bien"
        print(f"{name:<16} {state}")
    for extra in sorted(present - repos.keys()):
        print(f"{extra:<16} no está en corpus.toml")


def main():
    commands = {"sync": sync, "record": record, "status": status}
    if len(sys.argv) != 2 or sys.argv[1] not in commands:
        sys.exit(__doc__)
    sys.stdout.reconfigure(encoding="utf-8")
    commands[sys.argv[1]]()


if __name__ == "__main__":
    main()
