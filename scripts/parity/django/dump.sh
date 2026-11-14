#!/usr/bin/env bash
# Vuelca el SQL que kerbo genera para cada migración de un repo del corpus en
# .parity/django/<repo>-kerbo/<app>/<migración>.sql, para cruzarlo con el de sqlmigrate (diff.py).
# Uso: scripts/parity/django/dump.sh [repo]
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
corpus_repo="${1:-django}"
apps_root="$(python "${here}/compare.py" --apps-root "${corpus_repo}" | tr -d '\r')"
"${here}/../dump_migrations_sql.sh" django "${corpus_repo}" dumps_the_sql_of_every_migration \
  "KERBO_DJANGO_APPS_ROOT=/c/${apps_root}" KERBO_DJANGO_SQL_OUTPUT=/o
