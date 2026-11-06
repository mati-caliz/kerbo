"""Imprime el cuerpo de las funciones de ShellCheck pedidas, con todas sus cláusulas y sin sus pruebas
prop_. Uso: python show_checks.py checkTr checkExpr"""
import os
import re
import sys

here = os.path.dirname(os.path.abspath(__file__))
source_root = os.path.join(here, '..', '..', '..', '..', 'kerbo-parity', 'shellcheck-src', 'src', 'ShellCheck')
SKIPPED_BETWEEN_CLAUSES = ('--', 'prop_')
for name in ('Analytics.hs', 'Checks/Commands.hs', 'Checks/ShellSupport.hs', 'ASTLib.hs', 'AnalyzerLib.hs', 'Data.hs'):
    lines = open(os.path.join(source_root, name), encoding='utf-8').read().split('\n')
    for function in sys.argv[1:]:
        for index, line in enumerate(lines):
            if not re.match(rf'^{function}\b', line):
                continue
            end = last = index + 1
            while end < len(lines):
                current = lines[end]
                belongs = current[:1] in (' ', '\t') or re.match(rf'^{function}\b', current)
                if belongs:
                    last = end + 1
                elif current and not current.startswith(SKIPPED_BETWEEN_CLAUSES):
                    break
                end += 1
            print('--', name)
            print('\n'.join(lines[index:last]).rstrip())
            break
