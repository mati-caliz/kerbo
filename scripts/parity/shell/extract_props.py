"""Escribe en testdata/<carpeta>/ un script por cada prop_<función> de ShellCheck (de su fuente en
kerbo-parity/shellcheck-src). Las pruebas sin shebang van con el del dialecto por defecto.

Uso: python extract_props.py checkBashisms bashisms
     python extract_props.py checkTr,checkExpr commands '#!/bin/bash'
Con varias funciones, cada archivo lleva el nombre de la suya delante."""
import ast
import os
import re
import sys

here = os.path.dirname(os.path.abspath(__file__))
source_root = os.path.join(here, '..', '..', '..', '..', 'kerbo-parity', 'shellcheck-src', 'src', 'ShellCheck')
functions, folder = sys.argv[1].split(','), sys.argv[2]
default_shebang = sys.argv[3] if len(sys.argv) > 3 else '#!/bin/sh'
QUOTED = r'("(?:[^"\\]|\\.)*")'
target = os.path.join(here, 'testdata', folder)
os.makedirs(target, exist_ok=True)
sources = []
for root, _, files in os.walk(source_root):
    sources += [open(os.path.join(root, name), encoding='utf-8').read() for name in files if name.endswith('.hs')]
for function in functions:
    pattern = re.compile(r'^prop_' + function + r'(\w*) = (?:verify|verifyNot|verifyTree|verifyNotTree) ' + function + ' ' + QUOTED, re.M)
    for text in sources:
        for suffix, literal in pattern.findall(text):
            script = ast.literal_eval(literal)
            if not script.startswith('#!'):
                script = default_shebang + '\n' + script
            label = suffix or '0'
            if len(functions) > 1:
                label = f'{function}_{label}'
            with open(os.path.join(target, f'{label}.sh'), 'w', encoding='utf-8', newline='\n') as handle:
                handle.write(script + '\n')
print(len(os.listdir(target)), 'scripts')
