use crate::i18n::catalog::RuleText;
use crate::rules::PythonRule;

// Textos de las reglas de plugins de flake8 y de pyflakes del conjunto por defecto de ruff 0.17 que no aparecen en el corpus.
pub const PLUGIN_RULE_TEXTS: [(PythonRule, RuleText); 18] = [
    (
        PythonRule::EXE001,
        RuleText::new(
            "Shebang in a file that is not executable",
            "add the executable bit or remove the shebang",
            "A shebang says the file is meant to run as a script, but without the executable bit the system will not run it.",
            "chmod +x script.py, or delete the #!/usr/bin/env python line.",
        ),
    ),
    (
        PythonRule::EXE002,
        RuleText::new(
            "Executable file without a shebang",
            "add a shebang or remove the executable bit",
            "Without a shebang the system does not know which interpreter to use, so the script fails or runs with the wrong one.",
            "Add #!/usr/bin/env python as the first line, or chmod -x module.py.",
        ),
    ),
    (
        PythonRule::EXE004,
        RuleText::new(
            "Whitespace before the shebang",
            "remove the whitespace before #!",
            "The system only reads #! when they are the first two characters of the file, so any space before it turns the shebang into a comment.",
            "  #!/usr/bin/env python becomes #!/usr/bin/env python.",
        ),
    ),
    (
        PythonRule::INT001,
        RuleText::new(
            "f-string inside a gettext call",
            "translate the template and format the result",
            "The f-string is filled in before the lookup, so the catalog searches for the finished text and finds nothing.",
            "_(f\"Hi {name}\") becomes _(\"Hi %s\") % name.",
        ),
    ),
    (
        PythonRule::INT003,
        RuleText::new(
            "printf-style formatting inside a gettext call",
            "format the translated string instead",
            "The % runs before the lookup, so the catalog searches for the filled-in text and finds nothing.",
            "_(\"Hi %s\" % name) becomes _(\"Hi %s\") % name.",
        ),
    ),
    (
        PythonRule::LOG002,
        RuleText::new(
            "getLogger called with __file__ or __cached__",
            "use __name__",
            "Logger names are dotted module names; a file path breaks the logger hierarchy and its configuration.",
            "logging.getLogger(__file__) becomes logging.getLogger(__name__).",
        ),
    ),
    (
        PythonRule::LOG009,
        RuleText::new(
            "Undocumented logging.WARN",
            "use logging.WARNING",
            "logging.WARN is an undocumented alias; the documented name is logging.WARNING.",
            "level=logging.WARN becomes level=logging.WARNING.",
        ),
    ),
    (
        PythonRule::LOG014,
        RuleText::new(
            "exc_info outside an exception handler",
            "remove exc_info or move the call into the except",
            "Outside an except there is no exception to attach, so the log shows NoneType: None.",
            "logging.warning(\"failed\", exc_info=True) outside an except becomes logging.warning(\"failed\").",
        ),
    ),
    (
        PythonRule::G010,
        RuleText::new(
            "Deprecated logging warn",
            "use warning",
            "warn is a deprecated alias of warning and Python warns every time it is called.",
            "logger.warn(\"x\") becomes logger.warning(\"x\").",
        ),
    ),
    (
        PythonRule::G101,
        RuleText::new(
            "extra key that clashes with a LogRecord attribute",
            "rename the key",
            "LogRecord raises KeyError when extra tries to overwrite one of its own attributes, so the log call fails.",
            "extra={\"name\": user} becomes extra={\"user_name\": user}.",
        ),
    ),
    (
        PythonRule::G202,
        RuleText::new(
            "Redundant exc_info in logging.exception",
            "remove exc_info",
            "logging.exception already logs the current exception, so exc_info=True adds nothing.",
            "logger.exception(\"failed\", exc_info=True) becomes logger.exception(\"failed\").",
        ),
    ),
    (
        PythonRule::PT010,
        RuleText::new(
            "pytest.raises without an expected exception",
            "pass the expected exception",
            "Without an exception type pytest.raises fails when the test runs.",
            "with pytest.raises(): becomes with pytest.raises(ValueError):.",
        ),
    ),
    (
        PythonRule::PT020,
        RuleText::new(
            "Deprecated pytest.yield_fixture",
            "use pytest.fixture",
            "pytest.fixture supports yield since pytest 3.0; yield_fixture is a deprecated alias that pytest 10 removes.",
            "@pytest.yield_fixture becomes @pytest.fixture.",
        ),
    ),
    (
        PythonRule::PT025,
        RuleText::new(
            "pytest.mark.usefixtures on a fixture",
            "remove the mark and request the fixture as a parameter",
            "Marks have no effect on fixtures, so the fixtures listed there are never set up.",
            "@pytest.mark.usefixtures(\"db\") on a fixture becomes a db parameter.",
        ),
    ),
    (
        PythonRule::PT026,
        RuleText::new(
            "pytest.mark.usefixtures without arguments",
            "remove the mark or list the fixtures",
            "A usefixtures mark with no fixture names does nothing.",
            "@pytest.mark.usefixtures() can be deleted.",
        ),
    ),
    (
        PythonRule::TC010,
        RuleText::new(
            "String inside a runtime X | Y union",
            "quote the whole union or drop the quotes",
            "Python evaluates the annotation and \"Foo\" | None raises TypeError, because a string does not support |.",
            "x: \"Foo\" | None becomes x: \"Foo | None\".",
        ),
    ),
    (
        PythonRule::F407,
        RuleText::new(
            "Undefined __future__ feature",
            "remove the import",
            "Importing a feature that __future__ does not define is a SyntaxError.",
            "from __future__ import braces never works.",
        ),
    ),
    (
        PythonRule::F622,
        RuleText::new(
            "Two starred targets in one assignment",
            "keep a single starred target",
            "Python cannot tell which starred name takes the remaining values, so it is a SyntaxError.",
            "*a, *b = items becomes first, *rest = items.",
        ),
    ),
];
