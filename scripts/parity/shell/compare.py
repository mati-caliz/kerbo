"""kerbo (out/kerbo.json) contra ShellCheck (out/shellcheck.json) por regla, archivo, línea y
columna, sólo en las reglas de ShellCheck que kerbo tiene. ShellCheck cuenta las columnas en
caracteres (el tab es uno) y kerbo en bytes: se convierten con la línea del archivo.

Uso: python compare.py [--no-directives] [reglas...]; con --no-directives las líneas se leen sin
las directivas `# shellcheck`, como en la corrida que las borra."""
import collections
import json
import os
import re
import sys

here = os.path.dirname(os.path.abspath(__file__))
corpus = os.environ.get('SHELL_CORPUS', os.path.join(here, '..', '..', '..', '..', 'kerbo-parity', 'shell-corpus'))
families = open(os.path.join(here, '..', '..', '..', 'src', 'rules', 'families', 'shell.rs'), encoding='utf-8').read()
known = set(re.findall(r'"(sc\d+)"', families))
lines_cache = {}
arguments = sys.argv[1:]
without_directives = '--no-directives' in arguments
arguments = [argument for argument in arguments if argument != '--no-directives']
DIRECTIVE = re.compile(rb'^[ 	]*#[ 	]*shellcheck[ 	]')


def byte_column(path, line, column):
    if path not in lines_cache:
        with open(os.path.join(corpus, path), 'rb') as handle:
            lines_cache[path] = handle.read().split(b'\n')
    text = lines_cache[path][line - 1].decode('utf-8', 'replace') if line - 1 < len(lines_cache[path]) else ''
    return len(text[:column - 1].encode('utf-8')) + 1 + max(0, column - 1 - len(text))


shellcheck = set()
for comment in json.load(open(os.path.join(here, 'out', 'shellcheck.json'), encoding='utf-8')):
    rule = f'sc{comment["code"]}'
    if rule in known:
        shellcheck.add((rule, comment['file'], comment['line'], byte_column(comment['file'], comment['line'], comment['column'])))
kerbo = set()
for finding in json.load(open(os.path.join(here, 'out', 'kerbo.json'), encoding='utf-8')):
    if finding['rule'] in known:
        kerbo.add((finding['rule'], finding['path'].removeprefix('./'), finding['line'], finding['column']))
rules = arguments or sorted(known)
totals = collections.Counter()
for rule in rules:
    expected = {key for key in shellcheck if key[0] == rule}
    reported = {key for key in kerbo if key[0] == rule}
    totals.update(coinciden=len(expected & reported), faltan=len(expected - reported), sobran=len(reported - expected))
    if expected or reported:
        print(rule, 'coinciden', len(expected & reported), 'faltan', len(expected - reported), 'sobran', len(reported - expected))
    for key in sorted(expected - reported)[:6]:
        print('  FALTA', *key[1:])
    for key in sorted(reported - expected)[:6]:
        print('  SOBRA', *key[1:])
print(dict(totals))
