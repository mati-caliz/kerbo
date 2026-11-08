"""Corre adentro del contenedor de reference.sh: configura Django con las apps del repo, saca el SQL de
`sqlmigrate` de cada migración y lo pasa por squawk.

Cada repo dice en REPOS qué apps instala y qué carpeta agrega al path. Una migración que no se puede
convertir (importa algo que falta, o consulta la base) queda anotada en errores.txt.
"""

import io
import json
import os
import pathlib
import subprocess
import sys

REPOS = {
    "django": {
        "path": "/repo",
        "apps": [
            "django.contrib.admin",
            "django.contrib.auth",
            "django.contrib.contenttypes",
            "django.contrib.sessions",
            "django.contrib.sites",
            "django.contrib.flatpages",
            "django.contrib.redirects",
        ],
    },
}
OUT = pathlib.Path("/out")


def configure(repo):
    sys.path.insert(0, REPOS[repo]["path"])
    import django
    from django.conf import settings

    settings.configure(
        INSTALLED_APPS=REPOS[repo]["apps"],
        DATABASES={
            "default": {
                "ENGINE": "django.db.backends.postgresql",
                "HOST": os.environ["DATABASE_HOST"],
                "NAME": "kerbo",
                "USER": "postgres",
                "PASSWORD": "kerbo",
            }
        },
        DEFAULT_AUTO_FIELD="django.db.models.BigAutoField",
        USE_TZ=True,
    )
    django.setup()


def migrations_in_order():
    from django.db import connection
    from django.db.migrations.loader import MigrationLoader

    loader = MigrationLoader(connection, ignore_no_migrations=True)
    plan = loader.graph.leaf_nodes()
    seen = []
    for leaf in plan:
        for node in loader.graph.forwards_plan(leaf):
            if node not in seen:
                seen.append(node)
    return seen


def sql_of(app, name):
    from django.core.management import call_command

    buffer = io.StringIO()
    call_command("sqlmigrate", app, name, stdout=buffer, no_color=True)
    return buffer.getvalue()


def main():
    repo = os.environ["REPO"]
    configure(repo)
    errors = []
    written = []
    for app, name in migrations_in_order():
        try:
            sql = sql_of(app, name)
        except Exception as problem:  # noqa: BLE001 - cada migración que falla se anota y se sigue
            errors.append(f"{app} {name}: {type(problem).__name__}: {problem}")
            continue
        target = OUT / app / f"{name}.sql"
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_text(sql, encoding="utf-8")
        written.append(target)
    (OUT / "errores.txt").write_text("\n".join(errors) + "\n", encoding="utf-8")
    report = subprocess.run(
        ["squawk", "--reporter", "json", *map(str, written)], capture_output=True, text=True, check=False
    )
    (OUT / "squawk.json").write_text(report.stdout, encoding="utf-8")
    found = json.loads(report.stdout or "[]")
    print(f"{len(written)} migraciones, {len(errors)} con error, {len(found)} avisos de squawk")


if __name__ == "__main__":
    main()
