#!/usr/bin/env bash
# Corre ShellCheck 0.11.0 sobre los scripts de list_files.py y deja out/shellcheck.json (json1).
# Con --no-directives, antes borra las directivas `# shellcheck` de una copia del corpus.
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
corpus="${SHELL_CORPUS:-$(cd "$here/../../../.." && pwd)/kerbo-parity/shell-corpus}"
mkdir -p "$here/out"
PYTHONIOENCODING=utf-8 python "$here/list_files.py" "$corpus" > "$here/out/files.txt"
MSYS_NO_PATHCONV=1 docker run --rm -v "$(cd "$corpus" && pwd -W):/c:ro" -v "$(cd "$here/out" && pwd -W):/o" \
  -v "$(cd "$here" && pwd -W)/shellcheck-inner.sh:/inner.sh:ro" koalaman/shellcheck-alpine:v0.11.0 sh /inner.sh "${1:-}"
python - "$here/out" <<'PY'
import json, os, sys
out = sys.argv[1]
comments = []
for line in open(os.path.join(out, 'shellcheck.raw'), encoding='utf-8'):
    if line.strip():
        comments += json.loads(line)['comments']
json.dump(comments, open(os.path.join(out, 'shellcheck.json'), 'w', encoding='utf-8'))
print(len(comments), 'avisos')
PY
