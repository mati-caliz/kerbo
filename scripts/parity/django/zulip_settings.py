"""Settings de zulip para sacar la referencia: los de desarrollo del repo, sin el archivo de secretos (el
repo se monta de solo lectura y no lo trae) y apuntando a la base del contenedor de reference.sh.
"""

import importlib
import os

os.environ["DISABLE_MANDATORY_SECRET_CHECK"] = "True"
importlib.import_module("zproject.config").secrets_file.read_dict(
    {"secrets": {"local_database_password": "kerbo"}}
)
globals().update(
    (name, value)
    for name, value in vars(importlib.import_module("zproject.settings")).items()
    if name.isupper()
)
DATABASES = globals()["DATABASES"]
DATABASES["default"].update(HOST=os.environ["DATABASE_HOST"], NAME="kerbo", USER="postgres", PASSWORD="kerbo")
LOGGING_CONFIG = None
TEST_SUITE = True
LOG_DIRECTORY = "/tmp/zulip-log"
LOG_PATH_SUFFIX = "_LOG_PATH"
os.makedirs(LOG_DIRECTORY, exist_ok=True)
globals().update(
    (name, os.path.join(LOG_DIRECTORY, os.path.basename(value)))
    for name, value in list(globals().items())
    if name.endswith(LOG_PATH_SUFFIX) and isinstance(value, str)
)
DEVELOPMENT_LOG_DIRECTORY = LOG_DIRECTORY
