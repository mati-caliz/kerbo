"""Arma un fixture de src/shell/fixtures juntando scripts de testdata/<carpeta> (sin su shebang) y
escribe al lado la salida de ShellCheck 0.11.0 en las reglas que kerbo tiene con el prefijo dado,
como `línea:columna código`, igual que flow.shellcheck.

Uso: MSYS_NO_PATHCONV=1 python fixture.py nombre sc30 '#!/bin/sh' bashisms-more/local.sh ...
Se pueden dar varios prefijos separados por comas, o `!sc2004,sc2005,...`: todas las reglas de kerbo menos ésas y las que empiezan con
lo que se liste con `!` y termine en `*` (por ejemplo `!sc30*`). Sin scripts conserva el fixture y
sólo vuelve a sacar la salida de ShellCheck."""
import json
import os
import re
import subprocess
import sys

here = os.path.dirname(os.path.abspath(__file__))
fixtures = os.path.join(here, '..', '..', '..', 'src', 'shell', 'fixtures')
families = open(os.path.join(here, '..', '..', '..', 'src', 'rules', 'families', 'shell.rs'), encoding='utf-8').read()
known = set(re.findall(r'"sc(\d+)"', families))
DIRECTIVE = re.compile(r'^\s*#\s*shellcheck\s')
def shellcheck_image():
    versions = os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', 'versions.py')
    return subprocess.run([sys.executable, versions, 'image', 'shellcheck'], capture_output=True, text=True,
                          check=True).stdout.strip()


name, prefix, shebang, sources = sys.argv[1], sys.argv[2], sys.argv[3], sys.argv[4:]
script_path = os.path.join(fixtures, f'{name}.sh')
if sources:
    body = shebang + '\n'
    for source in sources:
        text = open(os.path.join(here, 'testdata', source), encoding='utf-8').read()
        text = text.split('\n', 1)[1] if text.startswith('#!') else text
        body += ''.join(line for line in text.splitlines(keepends=True) if not DIRECTIVE.match(line))
    with open(script_path, 'w', encoding='utf-8', newline='\n') as handle:
        handle.write(body)
else:
    body = open(script_path, encoding='utf-8', newline='').read()
folder = subprocess.run(['cygpath', '-w', os.path.abspath(fixtures)], capture_output=True, text=True).stdout.strip()
output = subprocess.run(
    ['docker', 'run', '--rm', '-v', f'{folder}:/f:ro', shellcheck_image(),
     'shellcheck', '--norc', '-f', 'json1', f'/f/{name}.sh'],
    capture_output=True, text=True, env=dict(os.environ, MSYS_NO_PATHCONV='1'),
).stdout
lines = body.split('\n')


def selected(code):
    if not prefix.startswith('!'):
        return code.startswith(tuple(prefix.split(',')))
    excluded = prefix[1:].split(',')
    return not any(code.startswith(item[:-1]) if item.endswith('*') else code == item for item in excluded)


expected = []
for comment in json.loads(output)['comments']:
    if str(comment['code']) in known and selected(f'sc{comment["code"]}'):
        text = lines[comment['line'] - 1]
        column = len(text[:comment['column'] - 1].encode('utf-8')) + 1
        expected.append((comment['line'], column, f'sc{comment["code"]}'))
with open(os.path.join(fixtures, f'{name}.shellcheck'), 'w', encoding='utf-8', newline='\n') as handle:
    handle.writelines(f'{line}:{column} {code}\n' for line, column, code in sorted(expected))
print(len(expected), 'avisos')
