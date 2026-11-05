"""Lista las funciones check de los fuentes de ShellCheck y los códigos que emite cada una que kerbo
todavía no tiene, con la cantidad de pruebas prop_ de cada función."""
import os
import re

here = os.path.dirname(os.path.abspath(__file__))
source_root = os.path.join(here, '..', '..', '..', '..', 'kerbo-parity', 'shellcheck-src', 'src', 'ShellCheck')
families = open(os.path.join(here, '..', '..', '..', 'src', 'rules', 'families', 'shell.rs'), encoding='utf-8').read()
known = set(re.findall(r'"sc(\d+)"', families))
EMIT = re.compile(r'\b(?:warn|err|info|style|warnWithFix|styleWithFix|errWithFix|infoWithFix|makeComment\s+\w+)\s+(?:\(?[\w.]+(?:\s+[\w.]+)*\)?\s+)?(\d{4})\b')
for name in ('Analytics.hs', 'Checks/Commands.hs', 'Checks/ControlFlow.hs', 'Checks/ShellSupport.hs', 'Parser.hs'):
    text = open(os.path.join(source_root, name), encoding='utf-8').read()
    current = None
    found = {}
    for line in text.split('\n'):
        match = re.match(r'^(\w+)\s.*=', line) or re.match(r'^(\w+) ::', line)
        if match and not match.group(1).startswith('prop_'):
            current = match.group(1)
        for code in EMIT.findall(line):
            if code not in known:
                found.setdefault(current, set()).add(code)
    print('==', name)
    for function, codes in found.items():
        props = len(re.findall(rf'^prop_{function}\w* =', text, re.M))
        print(function, props, ' '.join(sorted(codes)))
