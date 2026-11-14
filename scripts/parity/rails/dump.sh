#!/usr/bin/env bash
# Vuelca el SQL que kerbo genera para cada migración de un repo del corpus en
# .parity/rails/<repo>-kerbo/<migración>.sql, para cruzarlo con el de reference.sh.
# Uso: scripts/parity/rails/dump.sh [repo]
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
corpus_repo="${1:-maybe}"
"${here}/../dump_migrations_sql.sh" rails "${corpus_repo}" dumps_the_rails_sql_of_every_migration \
  "KERBO_RAILS_MIGRATE=/c/${corpus_repo}/db/migrate" KERBO_RAILS_SQL_OUTPUT=/o
