"""Imprime el cuerpo de las funciones de ShellCheck pedidas, sin sus pruebas prop_.
Uso: python show_checks.py checkTr checkExpr"""
import os
import re
import sys

here = os.path.dirname(os.path.abspath(__file__))
source_root = os.path.join(here, '..', '..', '..', '..', 'kerbo-parity', 'shellcheck-src', 'src', 'ShellCheck')
for name in ('Analytics.hs', 'Checks/Commands.hs', 'Checks/ShellSupport.hs', 'ASTLib.hs', 'AnalyzerLib.hs'):
    lines = open(os.path.join(source_root, name), encoding='utf-8').read().split('\n')
    for function in sys.argv[1:]:
        for index, line in enumerate(lines):
            if re.match(rf'^{function}\b.*(=|::)', line) or re.match(rf'^{function} *$', line):
                end = index + 1
                while end < len(lines) and (not lines[end] or lines[end][0] in ' \t' or re.match(rf'^{function}\b', lines[end])):
                    end += 1
                print('--', name)
                print('\n'.join(lines[index:end]).rstrip())
                break
