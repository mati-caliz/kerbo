use crate::i18n::catalog::RuleText;
use crate::rules::PythonRule;

// Más reglas del conjunto por defecto de ruff 0.17: modismos de pylint, refurb, flake8-logging y
// compañía.
pub const IDIOM_TEXTS: [(PythonRule, RuleText); 39] = [
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
    (
        PythonRule::UP034,
        RuleText::new(
            "Extra parentheses around an expression",
            "remove the duplicated parentheses",
            "A second pair of parentheses adds nothing and makes the reader look for a tuple or a precedence that is not there.",
            "print((x)) becomes print(x).",
        ),
    ),
    (
        PythonRule::PLC0205,
        RuleText::new(
            "__slots__ assigned a single string",
            "use a tuple or list of names, like (\"name\",)",
            "A string is iterated character by character, so __slots__ = \"name\" declares the slots n, a, m and e.",
            "__slots__ = \"name\" becomes __slots__ = (\"name\",).",
        ),
    ),
    (
        PythonRule::PLW0120,
        RuleText::new(
            "else on a loop that never breaks",
            "remove the else and dedent its body",
            "Without a break the else always runs, so it reads as a condition that does not exist.",
            "for x in y:
    f(x)
else:
    g() becomes the loop followed by g().",
        ),
    ),
    (
        PythonRule::RET501,
        RuleText::new(
            "return None in a function that only returns None",
            "write a plain return",
            "When a function never returns a value, an explicit None suggests a result that callers should not rely on.",
            "return None becomes return.",
        ),
    ),
    (
        PythonRule::PLR1711,
        RuleText::new(
            "Useless return at the end of a function",
            "remove the return",
            "A function already returns None when it reaches its end, so the last return only adds a line.",
            "def f():\n    g()\n    return becomes def f():\n    g().",
        ),
    ),
    (
        PythonRule::PLR0402,
        RuleText::new(
            "import a.b as b instead of from a import b",
            "write from a import b",
            "from a import b says the same with less repetition and is the form readers expect.",
            "import os.path as path becomes from os import path.",
        ),
    ),
    (
        PythonRule::B026,
        RuleText::new(
            "Star argument after a keyword argument",
            "move the *args before the keyword arguments",
            "Python passes *args before the keyword arguments even when it is written after them, which misleads about the order.",
            "f(a=1, *rest) becomes f(*rest, a=1).",
        ),
    ),
    (
        PythonRule::UP039,
        RuleText::new(
            "Empty parentheses in a class definition",
            "remove the parentheses",
            "class C(): and class C: are the same; the parentheses suggest a base class that is not there.",
            "class C(): becomes class C:.",
        ),
    ),
    (
        PythonRule::UP020,
        RuleText::new(
            "io.open instead of the builtin open",
            "use open",
            "Since Python 3 the builtin open is io.open, so the import adds nothing.",
            "io.open(\"f\") becomes open(\"f\").",
        ),
    ),
    (
        PythonRule::RUF032,
        RuleText::new(
            "Decimal built from a float literal",
            "pass the number as a string",
            "The float is rounded before Decimal sees it, so Decimal(0.1) is not 0.1.",
            "Decimal(0.1) becomes Decimal(\"0.1\").",
        ),
    ),
    (
        PythonRule::TRY201,
        RuleText::new(
            "raise error inside its own except",
            "write a plain raise",
            "A plain raise re-raises the same exception without adding this line to the traceback.",
            "except ValueError as error:\n    raise error becomes raise.",
        ),
    ),
    (
        PythonRule::UP012,
        RuleText::new(
            "encode(\"utf-8\") on a literal string",
            "write a bytes literal, or call encode() without the encoding",
            "UTF-8 is already the default, and an ASCII literal can be written as bytes directly.",
            "\"abc\".encode(\"utf-8\") becomes b\"abc\".",
        ),
    ),
    (
        PythonRule::C400,
        RuleText::new(
            "Generator passed to list() instead of a list comprehension",
            "write a list comprehension",
            "A comprehension builds the list directly and reads as a list at a glance.",
            "list(x * 2 for x in y) becomes [x * 2 for x in y].",
        ),
    ),
    (
        PythonRule::C409,
        RuleText::new(
            "tuple() around a list or tuple literal",
            "write the tuple literal",
            "The literal already says it is a tuple without building an intermediate collection.",
            "tuple([1, 2]) becomes (1, 2).",
        ),
    ),
    (
        PythonRule::C413,
        RuleText::new(
            "list() or reversed() around sorted()",
            "use sorted() alone, with reverse=True if needed",
            "sorted() already returns a new list, and it can sort in reverse order by itself.",
            "list(sorted(x)) becomes sorted(x).",
        ),
    ),
    (
        PythonRule::C419,
        RuleText::new(
            "Comprehension inside any() or all()",
            "pass a generator instead",
            "A generator lets any() and all() stop at the first decisive element instead of building the whole list.",
            "any([x > 0 for x in y]) becomes any(x > 0 for x in y).",
        ),
    ),
    (
        PythonRule::T100,
        RuleText::new(
            "Debugger call or import left in the code",
            "remove the debugger before committing",
            "A breakpoint left behind stops the program waiting for input wherever it runs.",
            "import pdb; pdb.set_trace() is removed.",
        ),
    ),
    (
        PythonRule::B017,
        RuleText::new(
            "assertRaises(Exception) or pytest.raises(Exception)",
            "expect the specific exception, or add match=",
            "Any error passes the test, including a typo in the code under test.",
            "pytest.raises(Exception) becomes pytest.raises(ValueError).",
        ),
    ),
    (
        PythonRule::B002,
        RuleText::new(
            "++x or --x, which does not increment",
            "write x += 1 or x -= 1",
            "Python has no increment operator: ++x is just +(+x) and leaves x unchanged.",
            "++count becomes count += 1.",
        ),
    ),
    (
        PythonRule::PLW0127,
        RuleText::new(
            "Variable assigned to itself",
            "remove the assignment",
            "x = x does nothing, and usually hides a typo in one of the two names.",
            "value = value is removed.",
        ),
    ),
    (
        PythonRule::PLW0133,
        RuleText::new(
            "Exception built but never raised",
            "add raise in front",
            "Creating an exception without raising it does nothing, so the error goes unnoticed.",
            "ValueError(\"bad\") becomes raise ValueError(\"bad\").",
        ),
    ),
];
