"""Para las reglas pedidas, separa lo que falta en kerbo.json entre archivos que kerbo no analiza (no
dan ningún aviso) y los demás, que son los que hay que mirar; y lista lo que sobra."""
import json
import os
import sys

out = os.path.join(os.path.dirname(os.path.abspath(__file__)), 'out')
rules = set(sys.argv[1:])
kerbo = json.load(open(os.path.join(out, 'kerbo.json'), encoding='utf-8'))
analyzed = {finding['path'].removeprefix('./') for finding in kerbo}
found = {(finding['rule'], finding['path'].removeprefix('./'), finding['line']) for finding in kerbo}
expected = {(f'sc{comment["code"]}', comment['file'], comment['line'])
            for comment in json.load(open(os.path.join(out, 'shellcheck.json'), encoding='utf-8'))}
missing = sorted(key for key in expected - found if key[0] in rules)
print('faltan', len(missing), 'en archivos sin analizar', sum(key[1] not in analyzed for key in missing))
for key in missing:
    if key[1] in analyzed:
        print('FALTA', *key)
for key in sorted(key for key in found - expected if key[0] in rules):
    print('SOBRA', *key)
