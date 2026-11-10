"""Configuración de sentry para reference.py: la de server.py con la base del contenedor de reference.sh
y sin los servicios (Redis, Kafka, Snuba) que `sqlmigrate` no usa."""

import os

from sentry.conf.server import *  # noqa: F403 - el archivo de configuración de sentry extiende la de server.py

DATABASES = {
    "default": {
        "ENGINE": "sentry.db.postgres",
        "NAME": "kerbo",
        "USER": "postgres",
        "PASSWORD": "kerbo",
        "HOST": os.environ["DATABASE_HOST"],
        "PORT": "",
        "AUTOCOMMIT": True,
        "ATOMIC_REQUESTS": False,
    }
}
SECRET_KEY = "kerbo"
SENTRY_CACHE = "sentry.cache.django.DjangoCache"
SENTRY_CACHE_OPTIONS = {}
CACHES = {
    "default": {"BACKEND": "django.core.cache.backends.locmem.LocMemCache"},
    "nodedata": {"BACKEND": "django.core.cache.backends.locmem.LocMemCache"},
}
SENTRY_OPTIONS.update(  # noqa: F405 - viene de server.py
    {
        "system.secret-key": "kerbo" * 11,
        "system.url-prefix": "http://kerbo",
        "redis.clusters": {"default": {"hosts": {0: {"db": 0}}}},
    }
)
