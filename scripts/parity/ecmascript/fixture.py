"""Corre ESLint sobre un fixture de src/lint/ecmascript/fixtures y escribe al lado su salida, como
`línea:columna regla` y con las columnas en bytes, igual que los demás `.eslint`.

Usa el ESLint y la configuración de .parity/eslint: sólo las reglas de `recommended` que kerbo
tiene (kerbo_rules.txt, que se regenera de src/rules/families/eslint.rs).

Los archivos `*_sloppy.js` se leen con el parser de ESLint en modo script: el de typescript-eslint rechaza
`with` y los octales, que es justo lo que miden esas reglas.

Uso: python fixture.py archivo.js [archivo.ts ...]"""
import json
import os
import re
import shutil
import subprocess
import sys

here = os.path.dirname(os.path.abspath(__file__))
repo = os.path.join(here, '..', '..', '..')
fixtures = os.path.join(repo, 'src', 'lint', 'ecmascript', 'fixtures')
eslint_dir = os.path.abspath(os.path.join(os.environ.get('KERBO_PARITY', os.path.join(repo, '.parity')), 'eslint'))
scratch = os.path.join(eslint_dir, 'fixture')
ERROR_MARK = 'FATAL'
SLOPPY_SUFFIX = '_sloppy.js'
SLOPPY_CONFIG = '''import js from "@eslint/js";
import { readFileSync } from "node:fs";
const kerboRules = new Set(JSON.parse(`[${readFileSync(new URL("./kerbo_rules.txt", import.meta.url), "utf8")}]`));
export default [{
  languageOptions: { sourceType: "script", ecmaVersion: "latest" },
  linterOptions: { noInlineConfig: true, reportUnusedDisableDirectives: "off" },
  rules: Object.fromEntries(Object.entries({ ...js.configs.recommended.rules, "no-var": "error", "prefer-const": "error", "prefer-rest-params": "error", "prefer-spread": "error" }).filter(([name]) => kerboRules.has(name))),
}];
'''


def write_rule_list():
    families = open(os.path.join(repo, 'src', 'rules', 'families', 'eslint.rs'), encoding='utf-8').read()
    names = re.findall(r'=> "([^"]+)"', families)
    with open(os.path.join(eslint_dir, 'kerbo_rules.txt'), 'w', encoding='utf-8', newline='\n') as handle:
        handle.write(','.join(json.dumps(name) for name in names))


def byte_column(lines, line, column):
    text = lines[line - 1] if line - 1 < len(lines) else ''
    prefix = text.encode('utf-16-le')[: (column - 1) * 2].decode('utf-16-le', 'ignore')
    return len(prefix.encode('utf-8')) + 1


def run(name):
    source = os.path.join(fixtures, name)
    target = os.path.join(scratch, name)
    os.makedirs(scratch, exist_ok=True)
    shutil.copyfile(source, target)
    config = ''
    if name.endswith(SLOPPY_SUFFIX):
        with open(os.path.join(eslint_dir, 'sloppy.config.mjs'), 'w', encoding='utf-8', newline='\n') as handle:
            handle.write(SLOPPY_CONFIG)
        config = '-c sloppy.config.mjs '
    command = f'npx eslint {config}--no-warn-ignored -f json "fixture/{name}"'
    result = subprocess.run(command, cwd=eslint_dir, capture_output=True, text=True, encoding='utf-8', shell=True)
    report = json.loads(result.stdout)[0]
    lines = open(source, encoding='utf-8', newline='').read().split('\n')
    found = []
    for message in report['messages']:
        if message.get('fatal'):
            found = [(0, 0, ERROR_MARK + ' ' + message['message'])]
            break
        rule = (message.get('ruleId') or '').replace('@typescript-eslint/', '')
        found.append((message['line'], byte_column(lines, message['line'], message['column']), rule))
    golden = os.path.join(fixtures, os.path.splitext(name)[0] + '.eslint')
    with open(golden, 'w', encoding='utf-8', newline='\n') as handle:
        handle.writelines(f'{line}:{column} {rule}\n' for line, column, rule in sorted(found))
    print(name, len(found), 'avisos')


write_rule_list()
for fixture in sys.argv[1:]:
    run(fixture)
