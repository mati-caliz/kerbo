"""Compara probe-shellcheck.json con probe-kerbo.json (de probe.sh) en los códigos con el prefijo
dado, por archivo, línea y columna (las de ShellCheck pasadas a bytes)."""
import json
import os
import sys

out, data, prefix = sys.argv[1], sys.argv[2], sys.argv[3]


def byte_column(path, line, column):
    with open(os.path.join(data, path), 'rb') as handle:
        lines = handle.read().split(b'\n')
    text = lines[line - 1].decode('utf-8', 'replace') if line - 1 < len(lines) else ''
    return len(text[:column - 1].encode('utf-8')) + 1 + max(0, column - 1 - len(text))


shellcheck = {}
for comment in json.load(open(os.path.join(out, 'probe-shellcheck.json'), encoding='utf-8'))['comments']:
    rule = f'sc{comment["code"]}'
    if rule.startswith(prefix):
        key = (comment['file'], comment['line'], byte_column(comment['file'], comment['line'], comment['column']), rule)
        shellcheck[key] = comment['message']
kerbo = set()
for finding in json.load(open(os.path.join(out, 'probe-kerbo.json'), encoding='utf-8')):
    if finding['rule'].startswith(prefix):
        kerbo.add((finding['path'].removeprefix('./'), finding['line'], finding['column'], finding['rule']))
for key in sorted(set(shellcheck) | kerbo, key=lambda item: (item[0], item[1], item[2], item[3])):
    mark = '  ' if key in shellcheck and key in kerbo else ('FALTA' if key in shellcheck else 'SOBRA')
    print(mark, key[3], f'{key[1]}:{key[2]}', key[0], shellcheck.get(key, ''))
matched = len(set(shellcheck) & kerbo)
print('coinciden', matched, 'faltan', len(shellcheck) - matched, 'sobran', len(kerbo) - matched)
