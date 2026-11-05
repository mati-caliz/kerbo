"""Lista, relativos a testdata/, los scripts de las carpetas pedidas que tree-sitter-bash parsea sin
errores: los únicos que sirven para un fixture, porque un error hace saltear el archivo entero."""
import os
import sys

import tree_sitter
import tree_sitter_bash

here = os.path.dirname(os.path.abspath(__file__))
parser = tree_sitter.Parser(tree_sitter.Language(tree_sitter_bash.language()))
for folder in sys.argv[1:]:
    root = os.path.join(here, 'testdata', folder)
    for name in sorted(os.listdir(root)):
        if name.endswith('.sh'):
            with open(os.path.join(root, name), 'rb') as handle:
                if not parser.parse(handle.read()).root_node.has_error:
                    print(f'{folder}/{name}')
