"""Avisos de staticcheck del helper (out/helper) contra la corrida del conjunto por defecto de
golangci-lint (out/default), por regla, archivo, línea y columna.

Uso: python compare_corpus.py [reglas...]; sin reglas, todas las que el helper reporta."""
import glob
import json
import os
import sys

out = os.path.join(os.path.dirname(os.path.abspath(__file__)), 'out')
golangci = set()
for path in glob.glob(os.path.join(out, 'default', '*.json')):
    text = open(path, encoding='utf-8').read().strip()
    if not text:
        continue
    for issue in json.loads(text).get('Issues') or []:
        if issue['FromLinter'] != 'staticcheck':
            continue
        code = issue['Text'].split(':')[0].split('(')[0].lower()
        position = issue['Pos']
        golangci.add((code, position['Filename'].split('/w/repos/')[-1], position['Line'], position['Column']))
helper = {}
for path in glob.glob(os.path.join(out, 'helper', '*.jsonl')):
    for line in open(path, encoding='utf-8'):
        facts = json.loads(line)
        for report in facts.get('reports') or []:
            key = (report['rule'], facts['file'].split('/w/repos/')[-1], report['line'], report['column'])
            helper[key] = report['subject']
rules = sys.argv[1:] or sorted({key[0] for key in helper} - {'unused'})
for rule in rules:
    expected = {key for key in golangci if key[0] == rule}
    reported = {key for key in helper if key[0] == rule}
    print(rule, 'coinciden', len(expected & reported), 'faltan', len(expected - reported), 'sobran', len(reported - expected))
    for key in sorted(expected - reported)[:8]:
        print('  FALTA', *key[1:])
    for key in sorted(reported - expected)[:8]:
        print('  SOBRA', *key[1:], helper[key])
