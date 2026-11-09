"""Avisos de staticcheck, stdmethods y unused del helper (out/helper) contra la corrida del conjunto por defecto de
golangci-lint (out/default), por regla, archivo, línea y columna.

Uso: python compare_corpus.py [reglas...]; sin reglas, todas las que el helper reporta."""
import glob
import json
import io
import os
import re
import subprocess
import sys
import tarfile

GOVET_RULE = 'stdmethods'
GENERATED_MARKER = re.compile(rb'^// Code generated .* DO NOT EDIT\.$', re.MULTILINE)
NOLINT_MARKER = re.compile(rb'//\s*nolint(?::[^\s]*)?')
STATICCHECK_LINTERS = (b'staticcheck', b'all')
HEADER_BYTES = 2048
out = os.path.join(os.path.dirname(os.path.abspath(__file__)), 'out')
golangci = set()
for path in glob.glob(os.path.join(out, 'default', '*.json')):
    text = open(path, encoding='utf-8').read().strip()
    if not text:
        continue
    for issue in json.loads(text).get('Issues') or []:
        linter = issue['FromLinter']
        if linter == 'staticcheck':
            code = issue['Text'].split(':')[0].split('(')[0].lower()
        elif linter == 'govet' and issue['Text'].startswith(GOVET_RULE):
            code = GOVET_RULE
        elif linter == 'unused':
            code = 'unused'
        else:
            continue
        position = issue['Pos']
        golangci.add((code, position['Filename'].split('/w/repos/')[-1], position['Line'], position['Column']))
helper = {}
for path in glob.glob(os.path.join(out, 'helper', '*.jsonl')):
    for line in open(path, encoding='utf-8'):
        facts = json.loads(line)
        for report in facts.get('reports') or []:
            key = (report['rule'], facts['file'].split('/w/repos/')[-1], report['line'], report['column'])
            helper[key] = report['subject']


def read_sources(files):
    listing = '\n'.join(sorted(files)).encode()
    archive = subprocess.run(
        ['docker', 'run', '--rm', '-i', '-v', 'go-parity:/w', 'alpine', 'tar', '-C', '/w/repos', '-cf', '-', '-T', '-'],
        input=listing, capture_output=True, env={**os.environ, 'MSYS_NO_PATHCONV': '1'}).stdout
    sources = {}
    with tarfile.open(fileobj=io.BytesIO(archive)) as tar:
        for member in tar.getmembers():
            if member.isfile():
                sources[member.name] = tar.extractfile(member).read()
    return sources


def has_nolint_for_staticcheck(line_bytes):
    for match in NOLINT_MARKER.finditer(line_bytes):
        linters = match.group(0).partition(b':')[2]
        if not linters or any(name in linters.split(b',') for name in STATICCHECK_LINTERS):
            return True
    return False


def explained_by_source(key, sources):
    source = sources.get(key[1])
    if source is None:
        return False
    if GENERATED_MARKER.search(source[:HEADER_BYTES]):
        return True
    lines = source.splitlines()
    return any(has_nolint_for_staticcheck(line) for line in lines[:key[2]])


rules = sys.argv[1:] or sorted({key[0] for key in helper})
sources = read_sources({key[1] for key in helper if key not in golangci})
for rule in rules:
    expected = {key for key in golangci if key[0] == rule}
    reported = {key for key in helper if key[0] == rule}
    extra = reported - expected
    unexplained = {key for key in extra if not explained_by_source(key, sources)}
    print(rule, 'coinciden', len(expected & reported), 'faltan', len(expected - reported), 'sobran', len(extra),
          'sin explicar', len(unexplained))
    for key in sorted(expected - reported)[:8]:
        print('  FALTA', *key[1:])
    for key in sorted(unexplained)[:8]:
        print('  SOBRA SIN EXPLICAR', *key[1:], helper[key])
