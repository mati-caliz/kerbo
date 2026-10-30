"""Arma un fixture de src/shell/fixtures juntando scripts de testdata/<carpeta> (sin su shebang) y
escribe al lado la salida de ShellCheck 0.11.0 en las reglas que kerbo tiene con el prefijo dado,
como `línea:columna código`, igual que flow.shellcheck.

Uso: MSYS_NO_PATHCONV=1 python fixture.py nombre sc30 '#!/bin/sh' bashisms-more/local.sh ..."""
import json
import os
import re
import subprocess
import sys

here = os.path.dirname(os.path.abspath(__file__))
fixtures = os.path.join(here, '..', '..', '..', 'src', 'shell', 'fixtures')
families = open(os.path.join(here, '..', '..', '..', 'src', 'rules', 'families', 'shell.rs'), encoding='utf-8').read()
known = set(re.findall(r'"sc(\d+)"', families))
name, prefix, shebang, sources = sys.argv[1], sys.argv[2], sys.argv[3], sys.argv[4:]
body = shebang + '\n'
for source in sources:
    text = open(os.path.join(here, 'testdata', source), encoding='utf-8').read()
    body += text.split('\n', 1)[1] if text.startswith('#!') else text
script_path = os.path.join(fixtures, f'{name}.sh')
with open(script_path, 'w', encoding='utf-8', newline='\n') as handle:
    handle.write(body)
folder = subprocess.run(['cygpath', '-w', os.path.abspath(fixtures)], capture_output=True, text=True).stdout.strip()
output = subprocess.run(
    ['docker', 'run', '--rm', '-v', f'{folder}:/f:ro', 'koalaman/shellcheck-alpine:v0.11.0',
     'shellcheck', '--norc', '-f', 'json1', f'/f/{name}.sh'],
    capture_output=True, text=True, env=dict(os.environ, MSYS_NO_PATHCONV='1'),
).stdout
lines = body.split('\n')
expected = []
for comment in json.loads(output)['comments']:
    if str(comment['code']) in known and f'sc{comment["code"]}'.startswith(prefix):
        text = lines[comment['line'] - 1]
        column = len(text[:comment['column'] - 1].encode('utf-8')) + 1
        expected.append((comment['line'], column, f'sc{comment["code"]}'))
with open(os.path.join(fixtures, f'{name}.shellcheck'), 'w', encoding='utf-8', newline='\n') as handle:
    handle.writelines(f'{line}:{column} {code}\n' for line, column, code in sorted(expected))
print(len(expected), 'avisos')
