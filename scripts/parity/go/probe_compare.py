"""Compara los avisos de staticcheck de golangci-lint y del helper sobre el módulo de prueba."""
import json
import os
import sys

golangci_path, helper_path = sys.argv[1], sys.argv[2]
golangci = set()
for issue in json.load(open(golangci_path, encoding='utf-8')).get('Issues') or []:
    if issue['FromLinter'] != 'staticcheck':
        continue
    code = issue['Text'].split(':')[0].lower()
    position = issue['Pos']
    golangci.add((code, os.path.basename(position['Filename']), position['Line'], position['Column']))
helper = set()
for line in open(helper_path, encoding='utf-8'):
    facts = json.loads(line)
    for report in facts.get('reports') or []:
        if report['rule'] == 'unused':
            continue
        helper.add((report['rule'], os.path.basename(facts['file']), report['line'], report['column']))
for entry in sorted(golangci | helper, key=lambda item: (item[1], item[2], item[3], item[0])):
    mark = '  ' if entry in golangci and entry in helper else ('FALTA' if entry in golangci else 'SOBRA')
    print(mark, entry[0], f'{entry[2]}:{entry[3]}', entry[1])
print('coinciden', len(golangci & helper), 'faltan', len(golangci - helper), 'sobran', len(helper - golangci))
