"""Muestra el código de cada aviso de más (o que falta) de las reglas pedidas: la línea en el corpus, sin
las directivas `# shellcheck` si la corrida las borró. Lee la salida de compare.py por comparar de nuevo.

Uso: python show_diffs.py [--no-directives] [--missing] [--limit N] sc2317 sc2218"""
import json
import os
import re
import sys

here = os.path.dirname(os.path.abspath(__file__))
corpus = os.environ.get('SHELL_CORPUS', os.path.join(here, '..', '..', '..', '.parity', 'shell-corpus'))
DIRECTIVE = re.compile(rb'^[ \t]*#[ \t]*shellcheck[ \t]')
arguments = sys.argv[1:]
without_directives = '--no-directives' in arguments
want_missing = '--missing' in arguments
limit = 20
if '--limit' in arguments:
    limit = int(arguments[arguments.index('--limit') + 1])
    arguments = [item for item in arguments if item != str(limit)]
rules = {item for item in arguments if item.startswith('sc')}
cache = {}


def lines_of(path):
    if path not in cache:
        with open(os.path.join(corpus, path), 'rb') as handle:
            lines = handle.read().split(b'\n')
        cache[path] = [line for line in lines if not DIRECTIVE.match(line)] if without_directives else lines
    return cache[path]


def byte_column(path, line, column):
    lines = lines_of(path)
    text = lines[line - 1].decode('utf-8', 'replace') if line - 1 < len(lines) else ''
    return len(text[:column - 1].encode('utf-8')) + 1 + max(0, column - 1 - len(text))


out = os.path.join(here, 'out')
expected = set()
for comment in json.load(open(os.path.join(out, 'shellcheck.json'), encoding='utf-8')):
    rule = f'sc{comment["code"]}'
    if rule in rules:
        expected.add((rule, comment['file'], comment['line'], byte_column(comment['file'], comment['line'], comment['column'])))
reported = set()
for finding in json.load(open(os.path.join(out, 'kerbo.json'), encoding='utf-8')):
    if finding['rule'] in rules:
        reported.add((finding['rule'], finding['path'].removeprefix('./'), finding['line'], finding['column']))
selected = sorted(expected - reported) if want_missing else sorted(reported - expected)
for rule, path, line, column in selected[:limit]:
    lines = lines_of(path)
    code = lines[line - 1].decode('utf-8', 'replace').strip() if line - 1 < len(lines) else ''
    print(rule, f'{path}:{line}:{column}', '|', code[:110])
print(len(selected), 'en total')
