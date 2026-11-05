"""Parte testdata/<carpeta>/shapes.sh en un script por caso (shape_NNN.sh), para que una línea que
tree-sitter no parsea no haga saltear las demás. Los bloques `case` y lo marcado entre líneas `#{` y
`#}` van juntos.

Uso: python split_shapes.py structure-more"""
import os
import sys

here = os.path.dirname(os.path.abspath(__file__))
folder = os.path.join(here, 'testdata', sys.argv[1])
lines = open(os.path.join(folder, 'shapes.sh'), encoding='utf-8').read().splitlines()
shebang, lines = lines[0], [line for line in lines[1:] if not line.startswith('# ')]
groups, current, closing = [], [], None
for line in lines:
    if closing is None:
        if line.startswith('case '):
            closing = 'esac'
        elif line == '#{':
            closing = '#}'
            continue
        else:
            groups.append([line])
            continue
    if line == closing:
        if closing == 'esac':
            current.append(line)
        groups.append(current)
        current, closing = [], None
    else:
        current.append(line)
for number, group in enumerate(groups):
    path = os.path.join(folder, f'shape_{number:03}.sh')
    with open(path, 'w', encoding='utf-8', newline='\n') as handle:
        handle.write('\n'.join([shebang, *group]) + '\n')
print(len(groups), 'scripts')
