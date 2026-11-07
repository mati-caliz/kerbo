"""kerbo (out/kerbo.json) contra ruff (out/ruff.json) por regla, archivo, línea y columna. ruff cuenta
las columnas en caracteres y kerbo en bytes: se convierten con la línea del archivo.

ruff se corre con todas las reglas que kerbo tiene, así RUF100 ve las mismas prendidas. Al comparar
quedan afuera EXE001 y EXE002, porque kerbo corre en Docker sobre un montaje de Windows donde todo
archivo parece ejecutable.

Uso: python compare.py [--select-codes] [reglas...]
"""
import collections
import json
import pathlib
import re
import sys

HERE = pathlib.Path(__file__).resolve().parent
REPO = HERE.parent.parent.parent
CORPUS = REPO / ".parity" / "corpus"
FAMILY_FILES = ("python.rs", "python_slots.rs")
EXCLUDED_RULES = {"exe001", "exe002"}
SHOWN_DIFFERENCES = 8


def known_rules():
    rules = set()
    for name in FAMILY_FILES:
        source = (REPO / "src" / "rules" / "families" / name).read_text(encoding="utf-8")
        rules.update(re.findall(r'=> "([a-z]+\d+)"', source))
    return rules


def read_lines(path, cache={}):
    if path not in cache:
        cache[path] = (CORPUS / path).read_bytes().split(b"\n")
    return cache[path]


def byte_column(path, line, column):
    lines = read_lines(path)
    text = lines[line - 1].decode("utf-8", "replace") if line - 1 < len(lines) else ""
    return len(text[: column - 1].encode("utf-8")) + 1 + max(0, column - 1 - len(text))


def relative_to_corpus(filename):
    normalized = filename.replace("\\", "/")
    marker = "/corpus/"
    return normalized.split(marker, 1)[1] if marker in normalized else normalized.removeprefix("./")


def load_ruff(known):
    found = set()
    for diagnostic in json.loads((HERE / "out" / "ruff.json").read_text(encoding="utf-8")):
        code = (diagnostic["code"] or "").lower()
        if code in known and code not in EXCLUDED_RULES:
            path = relative_to_corpus(diagnostic["filename"])
            location = diagnostic["location"]
            found.add((code, path, location["row"], byte_column(path, location["row"], location["column"])))
    return found


def load_kerbo(known):
    found = set()
    for finding in json.loads((HERE / "out" / "kerbo.json").read_text(encoding="utf-8")):
        rule = finding["rule"]
        if rule in known and rule not in EXCLUDED_RULES:
            found.add((rule, finding["path"].removeprefix("./"), finding["line"], finding["column"]))
    return found


def main():
    arguments = sys.argv[1:]
    known = known_rules()
    if "--select-codes" in arguments:
        print(",".join(sorted(code.upper() for code in known)))
        return
    ruff = load_ruff(known)
    kerbo = load_kerbo(known)
    missing = collections.defaultdict(list)
    extra = collections.defaultdict(list)
    for key in ruff - kerbo:
        missing[key[0]].append(key[1:])
    for key in kerbo - ruff:
        extra[key[0]].append(key[1:])
    selected = set(arguments) or set(missing) | set(extra)
    for rule in sorted(selected):
        print(rule, "faltan", len(missing[rule]), "sobran", len(extra[rule]))
        for key in sorted(missing[rule])[:SHOWN_DIFFERENCES]:
            print("  FALTA", *key)
        for key in sorted(extra[rule])[:SHOWN_DIFFERENCES]:
            print("  SOBRA", *key)
    print(f"coinciden {len(ruff & kerbo)} faltan {len(ruff - kerbo)} sobran {len(kerbo - ruff)}")
    files = collections.Counter(key[1] for key in ruff - kerbo)
    print("archivos con más faltantes:", files.most_common(10))


main()
