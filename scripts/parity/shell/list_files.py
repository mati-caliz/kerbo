"""Lista los scripts del corpus que kerbo analiza como shell: .sh, .bash y los que no tienen
extensión con shebang de sh, bash, dash o busybox (también por env). Rutas relativas al corpus."""
import os
import re
import sys

SHEBANG = re.compile(rb'^#!\s*(?:\S*/)?(?:env\s+(?:-\S+\s+)*)?(?:\S*/)?(sh|bash|dash|busybox)\b')
corpus = sys.argv[1]
for root, directories, files in os.walk(corpus):
    directories[:] = [name for name in directories if name != '.git']
    for name in sorted(files):
        path = os.path.join(root, name)
        extension = os.path.splitext(name)[1]
        if extension in ('.sh', '.bash'):
            selected = True
        elif extension == '':
            with open(path, 'rb') as handle:
                selected = bool(SHEBANG.match(handle.readline()))
        else:
            selected = False
        if selected and not os.path.islink(path):
            print(os.path.relpath(path, corpus).replace(os.sep, '/'))
