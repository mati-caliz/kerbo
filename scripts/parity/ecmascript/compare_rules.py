"""Compara las reglas pedidas entre out_rules.json (ESLint) y kerbo.json en .parity/eslint, por
archivo, línea, columna y regla. Lo de ESLint en archivos que kerbo no analiza se cuenta aparte, y no se
filtra por los archivos donde kerbo avisó: si no, lo que kerbo no ve quedaría escondido.

Uso: python compare_rules.py no-octal,no-with [límite]"""
import collections
import json
import os
import pathlib
import sys

here = os.path.dirname(os.path.abspath(__file__))
eslint_dir = pathlib.Path(here, '..', '..', '..', '.parity', 'eslint').resolve()
corpus = eslint_dir.parent / 'corpus'
rules = sys.argv[1].split(',')
limit = int(sys.argv[2]) if len(sys.argv) > 2 else 15
PREFIX = '@typescript-eslint/'


def byte_column(path, line, column, cache={}):
    if path not in cache:
        cache[path] = (corpus / path).read_bytes().decode('utf-8', 'replace').split('\n')
    text = cache[path][line - 1] if line - 1 < len(cache[path]) else ''
    prefix = text.encode('utf-16-le')[: (column - 1) * 2].decode('utf-16-le', 'ignore')
    return len(prefix.encode('utf-8')) + 1


found = collections.Counter()
fatal = set()
for result in json.load(open(eslint_dir / 'out_rules.json', encoding='utf-8')):
    path = pathlib.Path(result['filePath']).resolve().relative_to(corpus).as_posix()
    if any(message.get('fatal') for message in result['messages']):
        fatal.add(path)
        continue
    for message in result['messages']:
        name = (message.get('ruleId') or '').replace(PREFIX, '')
        if name in rules:
            found[(path, message['line'], byte_column(path, message['line'], message['column']), name)] += 1
kerbo = collections.Counter(
    (item['path'].removeprefix('./'), item['line'], item['column'], item['rule'])
    for item in json.load(open(eslint_dir / 'kerbo.json', encoding='utf-8'))
    if item['rule'] in rules
)
print('archivos que ESLint no parsea:', len(fatal))
for rule in rules:
    expected = collections.Counter({key: count for key, count in found.items() if key[3] == rule})
    reported = collections.Counter({key: count for key, count in kerbo.items() if key[3] == rule and key[0] not in fatal})
    print(rule, 'eslint', sum(expected.values()), 'kerbo', sum(reported.values()), 'iguales', sum((expected & reported).values()))
    for key in sorted(expected - reported)[:limit]:
        print('  de menos', *key[:3])
    for key in sorted(reported - expected)[:limit]:
        print('  de más', *key[:3])
