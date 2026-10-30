"""Suma reglas directas de staticcheck a kerbo: la variante de GoRule, el linter, la lista de
reglas directas y los textos en inglés y español (en i18n/<idioma>/go/staticcheck.rs).

Uso: python register.py reglas.json

reglas.json es una lista de {"variant": "Sa4004", "code": "sa4004", "en": [4 textos], "es": [4 textos]},
con los textos en el orden de RuleText::new: título, qué hacer, por qué y ejemplo.
"""
import io
import json
import os
import re
import sys

ROOT = os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', '..', '..')
CATALOG_HEADER = '''use crate::i18n::catalog::RuleText;
use crate::rules::GoRule;

// {comment}
pub const STATICCHECK_TEXTS: [(GoRule, RuleText); 0] = [
];
'''
COMMENTS = {
    'english': 'The staticcheck rules the type helper decides on its own.',
    'spanish': 'Las reglas de staticcheck que decide el helper de tipos.',
}


def read(path):
    return io.open(os.path.join(ROOT, path), encoding='utf-8', newline='').read()


def write(path, text):
    full = os.path.join(ROOT, path)
    os.makedirs(os.path.dirname(full), exist_ok=True)
    io.open(full, 'w', encoding='utf-8', newline='\n').write(text)


def replace_once(text, old, new, path):
    assert text.count(old) == 1, (path, old)
    return text.replace(old, new)


def rust_string(value):
    return json.dumps(value, ensure_ascii=False)


def register_family(rules):
    path = 'src/rules/families/go.rs'
    text = read(path)
    variants = ''.join(f'    {rule["variant"]} => "{rule["code"]}",\n' for rule in rules)
    text = replace_once(text, '});\n\nconst STATICCHECK', variants + '});\n\nconst STATICCHECK', path)
    arms = ''.join(f'\n            | Self::{rule["variant"]}' for rule in rules)
    text = replace_once(text, ' => STATICCHECK,', arms + ' => STATICCHECK,', path)
    write(path, text)


def register_scan(rules):
    path = 'src/scan/go_typed/direct.rs'
    text = read(path)
    count = int(re.search(r'DIRECT_RULES: \[GoRule; (\d+)\]', text).group(1))
    text = text.replace(f'DIRECT_RULES: [GoRule; {count}]', f'DIRECT_RULES: [GoRule; {count + len(rules)}]')
    entries = ''.join(f'    GoRule::{rule["variant"]},\n' for rule in rules)
    start = text.index('pub const DIRECT_RULES')
    end = text.index('];', start)
    text = text[:end] + entries + text[end:]
    write(path, text)


def register_texts(rules, language, key):
    path = f'src/i18n/{language}/go/staticcheck.rs'
    full = os.path.join(ROOT, path)
    text = read(path) if os.path.exists(full) else CATALOG_HEADER.format(comment=COMMENTS[language])
    count = int(re.search(r'\[\(GoRule, RuleText\); (\d+)\]', text).group(1))
    text = text.replace(f'[(GoRule, RuleText); {count}]', f'[(GoRule, RuleText); {count + len(rules)}]')
    entries = ''
    for rule in rules:
        lines = ''.join(f'            {rust_string(value)},\n' for value in rule[key])
        entries += f'    (\n        GoRule::{rule["variant"]},\n        RuleText::new(\n{lines}        ),\n    ),\n'
    end = text.rindex('];')
    write(path, text[:end] + entries + text[end:])


def main():
    rules = json.load(io.open(sys.argv[1], encoding='utf-8'))
    for rule in rules:
        assert len(rule['en']) == 4 and len(rule['es']) == 4, rule['code']
    register_family(rules)
    register_scan(rules)
    register_texts(rules, 'english', 'en')
    register_texts(rules, 'spanish', 'es')


if __name__ == '__main__':
    main()
