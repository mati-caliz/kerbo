"""Suma reglas de ShellCheck a kerbo: la variante de ShellRule, su severidad (advertencia si no se
dice otra) y los textos en inglés y español en i18n/<idioma>/shell/<catálogo>.rs.

Uso: python register.py reglas.json catálogo

reglas.json tiene {"why_constant": "BASHISM_WHY", "rules": [{"code": "sc3043", "severity": "Warning",
"en": [...], "es": [...]}]}, con los textos en el orden de RuleText::new (título, qué hacer, por qué,
ejemplo). Un `null` en el "por qué" usa why_constant, que se define en i18n/<idioma>/shell.rs.
"""
import io
import json
import os
import re
import sys

ROOT = os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', '..', '..')
COMMENTS = {
    'english': 'ShellCheck rules beyond the original set.',
    'spanish': 'Reglas de ShellCheck más allá del conjunto original.',
}


def read(path):
    return io.open(os.path.join(ROOT, path), encoding='utf-8', newline='').read()


def write(path, text):
    full = os.path.join(ROOT, path)
    os.makedirs(os.path.dirname(full), exist_ok=True)
    io.open(full, 'w', encoding='utf-8', newline='\n').write(text)


def variant(code):
    return code[0].upper() + code[1:]


def rust_string(value):
    return json.dumps(value, ensure_ascii=False)


def register_family(rules):
    path = 'src/rules/families/shell.rs'
    text = read(path)
    entries = ''.join(f'    {variant(rule["code"])} => "{rule["code"]}",\n' for rule in rules)
    marker = '});\n\npub const fn wrap_shell'
    assert text.count(marker) == 1
    write(path, text.replace(marker, entries + marker))


SEVERITY_LISTS = {'Error': 'ERRORS', 'Info': 'INFOS', 'Style': 'STYLES'}


def register_severity(rules):
    path = 'src/shell/severity.rs'
    text = read(path)
    for rule in rules:
        if rule['severity'] in SEVERITY_LISTS:
            start = text.index(f'const {SEVERITY_LISTS[rule["severity"]]}: &[ShellRule] = &[')
            end = text.index('];', start)
            separator = '' if text[:end].endswith('\n') else ',\n'
            text = text[:end] + separator + f'    ShellRule::{variant(rule["code"])},\n' + text[end:]
    write(path, text)


def register_texts(data, catalog, language, key):
    path = f'src/i18n/{language}/shell/{catalog}.rs'
    full = os.path.join(ROOT, path)
    constant = f'{catalog.upper()}_TEXTS'
    why = data.get('why_constant')
    if os.path.exists(full):
        text = read(path)
    else:
        shared = f'use super::{why};\n' if why else ''
        comment = data.get('comment', {}).get(language, COMMENTS[language])
        text = (f'{shared}use crate::i18n::catalog::RuleText;\nuse crate::rules::ShellRule;\n\n'
                f'// {comment}\n'
                f'pub const {constant}: [(ShellRule, RuleText); 0] = [\n];\n')
    count = int(re.search(r'\[\(ShellRule, RuleText\); (\d+)\]', text).group(1))
    text = text.replace(f'[(ShellRule, RuleText); {count}]', f'[(ShellRule, RuleText); {count + len(data["rules"])}]')
    entries = ''
    for rule in data['rules']:
        values = [rust_string(value) if value is not None else why for value in rule[key]]
        lines = ''.join(f'            {value},\n' for value in values)
        entries += f'    (\n        ShellRule::{variant(rule["code"])},\n        RuleText::new(\n{lines}        ),\n    ),\n'
    end = text.rindex('];')
    write(path, text[:end] + entries + text[end:])


def main():
    data = json.load(io.open(sys.argv[1], encoding='utf-8'))
    catalog = sys.argv[2]
    for rule in data['rules']:
        assert len(rule['en']) == 4 and len(rule['es']) == 4, rule['code']
    register_family(data['rules'])
    register_severity(data['rules'])
    register_texts(data, catalog, 'english', 'en')
    register_texts(data, catalog, 'spanish', 'es')


if __name__ == '__main__':
    main()
