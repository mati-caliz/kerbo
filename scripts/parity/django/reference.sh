#!/usr/bin/env bash
# Saca la referencia de las migraciones de Django de un repo del corpus: el SQL de `sqlmigrate` para
# Postgres de cada migración y lo que squawk marca sobre él. Deja .parity/django/<repo>/<app>/
# <migración>.sql y squawk.json.
#
#   scripts/parity/django/reference.sh django
#
# Las apps y el módulo de settings salen de settings.py al lado de este script: cada repo dice cuáles
# instala y desde dónde se importan.
set -euo pipefail

readonly CPUS=2
readonly MEMORY_LIMIT="4g"
readonly PYTHON_IMAGE="python:3.12-slim"
readonly POSTGRES_IMAGE="postgres:16-alpine"
DJANGO_VERSION="$(python "$(dirname "$0")/../versions.py" version django)"
SQUAWK_VERSION="$(python "$(dirname "$0")/../versions.py" version squawk)"
readonly DJANGO_VERSION SQUAWK_VERSION
repo="$1"
here="$(cd "$(dirname "$0")" && pwd)"
parity="$(cd "${KERBO_PARITY:-$here/../../../.parity}" && pwd)"
out="$parity/django/$repo"
network="kerbo-django-$repo"
database="kerbo-django-db-$repo"
mkdir -p "$out"
docker network create "$network" >/dev/null 2>&1 || true
docker rm -f "$database" >/dev/null 2>&1 || true
docker run -d --rm --name "$database" --network "$network" --cpus=1 -m 1g \
  -e POSTGRES_PASSWORD=kerbo -e POSTGRES_DB=kerbo "$POSTGRES_IMAGE" >/dev/null
trap 'docker rm -f "$database" >/dev/null 2>&1 || true' EXIT
MSYS_NO_PATHCONV=1 docker run --rm --network "$network" --cpus="$CPUS" -m "$MEMORY_LIMIT" \
  -v "$(cd "$parity/corpus/$repo" && pwd -W):/repo:ro" -v "$(cd "$out" && pwd -W):/out" \
  -v "$(cd "$here" && pwd -W):/harness:ro" -v kerbo-django-pip:/root/.cache/pip \
  -e REPO="$repo" -e DATABASE_HOST="$database" "$PYTHON_IMAGE" bash -euo pipefail -c "
    pip install -q 'django==$DJANGO_VERSION' 'psycopg[binary]' 'asgiref>=3.13' 'squawk-cli==$SQUAWK_VERSION' >/dev/null
    if [ -f /repo/requirements.txt ] && [ \"\$REPO\" != django ]; then pip install -q -r /repo/requirements.txt >/dev/null || true; fi
    for attempt in 1 2 3 4 5 6 7 8 9 10; do python -c 'import psycopg; psycopg.connect(\"host=$database user=postgres password=kerbo dbname=kerbo\")' 2>/dev/null && break; sleep 2; done
    cd /harness && python reference.py"
