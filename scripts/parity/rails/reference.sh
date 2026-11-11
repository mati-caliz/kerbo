#!/usr/bin/env bash
# Saca la referencia de las migraciones de Rails de un repo del corpus: el SQL que ActiveRecord manda a
# Postgres al correr cada migración de db/migrate (reference.rb) y lo que squawk marca sobre él. Deja
# .parity/rails/<repo>/<migración>.sql, errores.txt y squawk.json.
#
#   scripts/parity/rails/reference.sh chatwoot
set -euo pipefail

readonly CPUS=2
readonly MEMORY_LIMIT="4g"
readonly RUBY_IMAGE="ruby:3.4-slim"
readonly PYTHON_IMAGE="python:3.12-slim"
readonly POSTGRES_IMAGE="pgvector/pgvector:pg16"
ACTIVERECORD_VERSION="$(python "$(dirname "$0")/../versions.py" version activerecord)"
SQUAWK_VERSION="$(python "$(dirname "$0")/../versions.py" version squawk)"
readonly ACTIVERECORD_VERSION SQUAWK_VERSION
repo="$1"
here="$(cd "$(dirname "$0")" && pwd)"
parity="$(cd "${KERBO_PARITY:-$here/../../../.parity}" && pwd)"
out="$parity/rails/$repo"
network="kerbo-rails-$repo"
database="kerbo-rails-db-$repo"
rm -rf "$out"
mkdir -p "$out"
docker network create "$network" >/dev/null 2>&1 || true
docker rm -f "$database" >/dev/null 2>&1 || true
docker run -d --rm --name "$database" --network "$network" --cpus=1 -m 1g \
  -e POSTGRES_PASSWORD=kerbo -e POSTGRES_DB=kerbo "$POSTGRES_IMAGE" >/dev/null
trap 'docker rm -f "$database" >/dev/null 2>&1 || true' EXIT
MSYS_NO_PATHCONV=1 docker run --rm --network "$network" --cpus="$CPUS" -m "$MEMORY_LIMIT" \
  -v "$(cd "$parity/corpus/$repo" && pwd -W):/repo:ro" -v "$(cd "$out" && pwd -W):/out" \
  -v "$(cd "$here" && pwd -W):/harness:ro" -v kerbo-rails-gems:/usr/local/bundle \
  -e DATABASE_HOST="$database" "$RUBY_IMAGE" bash -euo pipefail -c "
    if ! gem list -i activerecord -v '$ACTIVERECORD_VERSION' >/dev/null; then
      apt-get update -qq >/dev/null && apt-get install -y -qq build-essential libpq-dev >/dev/null
      gem install -N activerecord -v '$ACTIVERECORD_VERSION' >/dev/null && gem install -N pg >/dev/null
    fi
    for attempt in 1 2 3 4 5 6 7 8 9 10; do ruby -rsocket -e 'TCPSocket.new(\"$database\", 5432)' 2>/dev/null && break; sleep 2; done
    sleep 2
    ruby /harness/reference.rb"
MSYS_NO_PATHCONV=1 docker run --rm --cpus=1 -m 1g -v "$(cd "$out" && pwd -W):/out" -v kerbo-django-pip:/root/.cache/pip \
  "$PYTHON_IMAGE" bash -euo pipefail -c "
    pip install -q 'squawk-cli==$SQUAWK_VERSION' >/dev/null 2>&1
    squawk --reporter json /out/*.sql > /out/squawk.json || true
    python -c 'import json; print(len(json.load(open(\"/out/squawk.json\"))), \"avisos de squawk\")'"
