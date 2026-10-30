"""Escribe en testdata/<carpeta>/ un script por cada prop_<función> de ShellCheck (de su fuente en
kerbo-parity/shellcheck-src), con `#!/bin/sh` si la prueba no trae shebang.

Uso: python extract_props.py checkBashisms bashisms"""
import ast
import os
import re
import sys

here = os.path.dirname(os.path.abspath(__file__))
source_root = os.path.join(here, '..', '..', '..', '..', 'kerbo-parity', 'shellcheck-src', 'src', 'ShellCheck')
function, folder = sys.argv[1], sys.argv[2]
QUOTED = r'("(?:[^"\\]|\\.)*")'
pattern = re.compile(r'^prop_' + function + r'(\w*) = (?:verify|verifyNot) ' + function + ' ' + QUOTED, re.M)
target = os.path.join(here, 'testdata', folder)
os.makedirs(target, exist_ok=True)
for root, _, files in os.walk(source_root):
    for name in files:
        if not name.endswith('.hs'):
            continue
        text = open(os.path.join(root, name), encoding='utf-8').read()
        for suffix, literal in pattern.findall(text):
            script = ast.literal_eval(literal)
            if not script.startswith('#!'):
                script = '#!/bin/sh\n' + script
            label = suffix or '0'
            with open(os.path.join(target, f'{label}.sh'), 'w', encoding='utf-8', newline='\n') as handle:
                handle.write(script + '\n')
print(len(os.listdir(target)), 'scripts')
