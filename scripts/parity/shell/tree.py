"""Imprime el árbol de tree-sitter-bash 0.25.1 de un script o de un texto, con tipo, posición
(línea:columna en base 1) y el texto de las hojas.

Uso: python tree.py archivo.sh | python tree.py -e 'echo ${x:1}'"""
import sys

import tree_sitter
import tree_sitter_bash

language = tree_sitter.Language(tree_sitter_bash.language())
parser = tree_sitter.Parser(language)
if sys.argv[1] == '-e':
    source = sys.argv[2].encode('utf-8')
else:
    source = open(sys.argv[1], 'rb').read()
tree = parser.parse(source)


def show(node, depth, field):
    label = f'{field}: ' if field else ''
    leaf = '' if node.child_count else ' ' + repr(source[node.start_byte:node.end_byte].decode('utf-8', 'replace'))
    kind = node.type if node.is_named else repr(node.type)
    print(f'{"  " * depth}{label}{kind} {node.start_point[0] + 1}:{node.start_point[1] + 1}{leaf}')
    for index, child in enumerate(node.children):
        show(child, depth + 1, node.field_name_for_child(index))


show(tree.root_node, 0, None)
