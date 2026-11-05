"""Frecuencia en out/shellcheck.json de los códigos de ShellCheck que kerbo todavía no tiene."""
import collections
import json
import os
import re

here = os.path.dirname(os.path.abspath(__file__))
families = open(os.path.join(here, '..', '..', '..', 'src', 'rules', 'families', 'shell.rs'), encoding='utf-8').read()
known = set(re.findall(r'"sc(\d+)"', families))
counts = collections.Counter()
samples = {}
for comment in json.load(open(os.path.join(here, 'out', 'shellcheck.json'), encoding='utf-8')):
    code = str(comment['code'])
    if code not in known:
        counts[code] += 1
        samples.setdefault(code, comment['message'][:90])
print('reglas de kerbo:', len(known), 'avisos sin regla:', sum(counts.values()), 'códigos:', len(counts))
for code, count in counts.most_common():
    print(f'SC{code} {count} {samples[code]}')
