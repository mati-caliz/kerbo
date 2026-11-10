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
MEMORY_LIMIT="4g"
PYTHON_IMAGE="python:3.12-slim"
readonly POSTGRES_IMAGE="postgres:16-alpine"
DJANGO_VERSION="$(python "$(dirname "$0")/../versions.py" version django)"
SQUAWK_VERSION="$(python "$(dirname "$0")/../versions.py" version squawk)"
readonly DJANGO_VERSION SQUAWK_VERSION
repo="$1"
here="$(cd "$(dirname "$0")" && pwd)"
parity="$(cd "${KERBO_PARITY:-$here/../../../.parity}" && pwd)"
out="$parity/django/$repo"
network="kerbo-django-$repo"
case "$repo" in
  zulip)
    dependency_groups="--only-group prod"
    database_driver=""
    system_packages="git build-essential pkg-config libpq-dev libldap2-dev libsasl2-dev libxml2-dev libxmlsec1-dev libicu-dev libvips42"
    ;;
  sentry)
    dependency_groups="--no-dev"
    database_driver=""
    system_packages="git build-essential pkg-config libpq-dev libxmlsec1-dev libxml2-dev libffi-dev"
    PYTHON_IMAGE="python:3.13-slim"
    MEMORY_LIMIT="8g"
    ;;
  wagtail)
    dependency_groups="--no-dev"
    database_driver="psycopg[binary] Jinja2"
    system_packages=""
    ;;
  *)
    dependency_groups="--no-dev"
    database_driver="psycopg[binary]"
    system_packages=""
    ;;
esac
readonly PYTHON_IMAGE MEMORY_LIMIT
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
  -e REPO="$repo" -e DATABASE_HOST="$database" \
  -e DEPENDENCY_GROUPS="$dependency_groups" -e SYSTEM_PACKAGES="$system_packages" \
  -e DATABASE_DRIVER="$database_driver" "$PYTHON_IMAGE" bash -euo pipefail -c "
    if [ \"\$REPO\" = django ]; then
      pip install -q 'django==$DJANGO_VERSION' 'psycopg[binary]' 'asgiref>=3.13' 'squawk-cli==$SQUAWK_VERSION' >/dev/null
    else
      apt-get update -qq >/dev/null && apt-get install -y -qq libmagic1 \$SYSTEM_PACKAGES >/dev/null
      pip install -q uv \$DATABASE_DRIVER 'squawk-cli==$SQUAWK_VERSION' >/dev/null
      (cd /repo && uv export --frozen --no-hashes --no-emit-project \$DEPENDENCY_GROUPS --project /repo) > /tmp/requirements.txt
      pip install -q -r /tmp/requirements.txt >/dev/null
    fi
    for attempt in 1 2 3 4 5 6 7 8 9 10; do python -c 'import socket; socket.create_connection((\"$database\", 5432), 1)' 2>/dev/null && break; sleep 2; done
    cd /harness && python reference.py"
