use crate::i18n::catalog::RuleText;
use crate::rules::PythonRule;

// Más reglas del conjunto por defecto de ruff 0.17: modismos de pylint, refurb, flake8-logging y
// compañía.
pub const IDIOM_TEXTS: [(PythonRule, RuleText); 18] = [
    (
        PythonRule::FURB167,
        RuleText::new(
            "A re flag with its one-letter alias",
            "use the full name, like re.IGNORECASE",
            "re.I or re.M force you to remember what each letter means; the full name reads by itself.",
            "re.I becomes re.IGNORECASE.",
        ),
    ),
    (
        PythonRule::PLR2044,
        RuleText::new(
            "An empty comment",
            "delete it, or write something",
            "A # with no text adds nothing and clutters the code.",
            "x = 1  # becomes x = 1.",
        ),
    ),
    (
        PythonRule::SIM201,
        RuleText::new(
            "An equality negated with not",
            "use a != b",
            "The negated comparison reads directly with !=, without a not in front.",
            "if not a == b: becomes if a != b:.",
        ),
    ),
    (
        PythonRule::PLW1510,
        RuleText::new(
            "subprocess.run without check",
            "pass check=True, or check=False if the error is handled by hand",
            "Without check, a failing command raises nothing and the program carries on as if it succeeded.",
            "subprocess.run(command) becomes subprocess.run(command, check=True).",
        ),
    ),
    (
        PythonRule::PLR0124,
        RuleText::new(
            "Something is compared with itself",
            "review the comparison: it was probably meant to be against something else",
            "x == x always gives the same result (except NaN): it's almost always a typo.",
            "if a == a: becomes if a == b:.",
        ),
    ),
    (
        PythonRule::PLR0133,
        RuleText::new(
            "Two constants are compared",
            "replace the comparison with its result, or review what was meant to be compared",
            "Comparing two literals always gives the same result: the condition decides nothing.",
            "if 1 == 1: becomes if True:, or the intended comparison.",
        ),
    ),
    (
        PythonRule::C401,
        RuleText::new(
            "set() over a generator",
            "use a set comprehension",
            "{x for x in y} does the same as set(x for x in y), shorter and without an extra call.",
            "set(x for x in y) becomes {x for x in y}.",
        ),
    ),
    (
        PythonRule::C402,
        RuleText::new(
            "dict() over a generator of pairs",
            "use a dict comprehension",
            "{k: v for ...} does the same as dict((k, v) for ...), clearer and without building intermediate tuples.",
            "dict((k, v) for k, v in pairs) becomes {k: v for k, v in pairs}.",
        ),
    ),
    (
        PythonRule::LOG015,
        RuleText::new(
            "Logging through the root logger",
            "use a module logger",
            "logging.info and friends write to the root logger: it can't be filtered or configured per module.",
            "logging.info(\"x\") becomes logger = logging.getLogger(__name__) and logger.info(\"x\").",
        ),
    ),
    (
        PythonRule::FLY002,
        RuleText::new(
            "A join over a fixed list of elements",
            "use an f-string",
            "\" \".join([a, b]) builds a list to glue two things an f-string writes directly.",
            "\" \".join([first, last]) becomes f\"{first} {last}\".",
        ),
    ),
    (
        PythonRule::PIE804,
        RuleText::new(
            "A literal dict unpacked with ** in a call",
            "pass the arguments by name",
            "f(**{\"a\": 1}) builds a dict to say the same as f(a=1).",
            "f(**{\"a\": 1}) becomes f(a=1).",
        ),
    ),
    (
        PythonRule::RUF010,
        RuleText::new(
            "str(), repr() or ascii() inside an f-string",
            "use the f-string conversion: !s, !r or !a",
            "The f-string already knows how to convert the value; the explicit call is longer and slower.",
            "f\"{repr(x)}\" becomes f\"{x!r}\".",
        ),
    ),
    (
        PythonRule::I001,
        RuleText::new(
            "The import block is not sorted or formatted",
            "sort it by section (standard, third party, first party, relative) and alphabetically, with a blank line between sections",
            "A fixed import order can be reviewed at a glance, avoids duplicates and removes merge conflicts from reorderings.",
            "import sys\nimport os becomes import os\nimport sys.",
        ),
    ),
    (
        PythonRule::UP032,
        RuleText::new(
            "str.format call that can be an f-string",
            "write it as an f-string",
            "An f-string puts each value where it is printed, so it reads in one pass and cannot pair a placeholder with the wrong argument.",
            "\"hello {}\".format(name) becomes f\"hello {name}\".",
        ),
    ),
    (
        PythonRule::N999,
        RuleText::new(
            "Module name is not a valid lowercase identifier",
            "rename the file in lowercase, with underscores and without starting with a digit",
            "A module whose name is not an identifier cannot be imported with a plain import, and PEP 8 asks for short lowercase names.",
            "MyModule.py becomes my_module.py.",
        ),
    ),
    (
        PythonRule::SIM103,
        RuleText::new(
            "if that returns True or False instead of returning the condition",
            "return the condition directly (with bool() or not if needed)",
            "Returning the condition says the same thing in one line and leaves no branch to read.",
            "if x > 0:
    return True
return False becomes return x > 0.",
        ),
    ),
    (
        PythonRule::SIM905,
        RuleText::new(
            "split() on a literal string instead of writing the list",
            "write the resulting list",
            "The list is visible as written, without computing it on every run or guessing where it splits.",
            "\"a b c\".split() becomes [\"a\", \"b\", \"c\"].",
        ),
    ),
    (
        PythonRule::FURB157,
        RuleText::new(
            "Decimal built from a string that could be a literal",
            "pass the number directly, or the special value as a string",
            "Decimal(10) says the same as Decimal(\"10\") without parsing a string, and Decimal(\"nan\") avoids going through a float.",
            "Decimal(\"10\") becomes Decimal(10).",
        ),
    ),
];
