from __future__ import annotations

import os
import sys
from collections import OrderedDict, defaultdict
from typing import TYPE_CHECKING, Any

import requests
from requests import Session

from . import sibling
from .. import parent
