"""errcheck con check-blank y check-type-assertions: golangci-lint contra lo que junta el helper.

El aviso de kerbo sale de `calls`, `receives` y `ranges` (ya medidos) más `blanks` y `assertions`; acá
se aplican las mismas exclusiones de fábrica que `src/scan/go_typed.rs`. Los avisos sin esas opciones
se descuentan comparando sólo las posiciones nuevas: lo de golangci-lint que está en `calls`,
`receives` o `ranges` no cuenta.

Uso: python compare_errcheck_options.py <carpeta de salida>"""
import glob
import json
import os
import re
import sys

out = sys.argv[1]
here = os.path.dirname(os.path.abspath(__file__))
source = open(os.path.join(here, '..', '..', '..', 'src', 'scan', 'go_typed.rs'), encoding='utf-8').read()
symbols = set(re.findall(r'^\s+"([^"]+)",$', source.split('EXCLUDED_SYMBOLS')[1].split('];')[0], re.M))


def excluded(call):
    if not call:
        return False
    if call.get('errors_as_type'):
        return True
    names = call.get('names') or []
    argument = call.get('argument') or ''
    return any(name in symbols or (argument and f'{name}({argument})' in symbols) for name in names)


def short(path):
    return path.split('/w/repos/')[-1]


plain = set()
new = {}
for path in glob.glob(os.path.join(out, 'helper', '*.jsonl')):
    for line in open(path, encoding='utf-8'):
        try:
            facts = json.loads(line)
        except ValueError:
            continue
        file = short(facts['file'])
        for kind in ('calls', 'receives', 'ranges'):
            for fact in facts.get(kind) or []:
                plain.add((file, fact['line'], fact['column']))
        for fact in facts.get('blanks') or []:
            if not excluded(fact.get('call')):
                new[(file, fact['line'], fact['column'])] = 'blank'
        for fact in facts.get('assertions') or []:
            new[(file, fact['line'], fact['column'])] = 'assertion'

golangci = {}
for path in glob.glob(os.path.join(out, 'golangci', '*.json')):
    text = open(path, encoding='utf-8').read().strip()
    if not text:
        continue
    for issue in json.loads(text).get('Issues') or []:
        if issue['FromLinter'] != 'errcheck':
            continue
        position = issue['Pos']
        key = (short(position['Filename']), position['Line'], position['Column'])
        golangci[key] = issue['Text']

expected = {key: text for key, text in golangci.items() if key in new or key not in plain}
print('golangci', len(golangci), 'nuevos esperados', len(expected), 'helper', len(new))
print('coinciden', len(set(expected) & set(new)))
missing = sorted(set(expected) - set(new))
extra = sorted(set(new) - set(golangci))
print('faltan', len(missing), 'sobran', len(extra))
for key in missing[:25]:
    print('  FALTA', *key, expected[key][:70])
for key in extra[:25]:
    print('  SOBRA', *key, new[key])
