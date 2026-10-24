use crate::i18n::catalog::RuleText;
use crate::rules::PythonRule;

// Más reglas del conjunto por defecto de ruff 0.17: sentencias, llamadas y pytest.
pub const MIXED_CHECK_TEXTS: [(PythonRule, RuleText); 24] = [
    (
        PythonRule::PLC0206,
        RuleText::new(
            "Dict iteration that indexes the value",
            "iterate dict.items()",
            "Looking up each key again inside the loop is slower and noisier than getting the key and value together.",
            "for k in d: print(d[k]) becomes for k, v in d.items(): print(v).",
        ),
    ),
    (
        PythonRule::PERF402,
        RuleText::new(
            "Loop that copies a list item by item",
            "use list(items) or result.extend(items)",
            "Appending every item one by one is slower than copying the iterable in a single call.",
            "for x in items: result.append(x) becomes result = list(items).",
        ),
    ),
    (
        PythonRule::PT031,
        RuleText::new(
            "pytest.warns with several statements",
            "leave only the statement that should warn inside the with",
            "If any other statement warns, the test passes even when the code under test does not.",
            "with pytest.warns(W): setup(); act() becomes setup() then with pytest.warns(W): act().",
        ),
    ),
    (
        PythonRule::PLW0602,
        RuleText::new(
            "global that is never assigned",
            "remove the global declaration",
            "global only matters to assign the module variable; to read it the declaration is not needed.",
            "global DEBUG followed only by reads of DEBUG is removed.",
        ),
    ),
    (
        PythonRule::PT014,
        RuleText::new(
            "Repeated case in pytest.mark.parametrize",
            "remove the repeated case",
            "The repeated case runs the same test twice without checking anything new.",
            "parametrize(\"x\", [1, 2, 1]) becomes parametrize(\"x\", [1, 2]).",
        ),
    ),
    (
        PythonRule::RUF019,
        RuleText::new(
            "Key check before indexing a dict",
            "use dict.get",
            "Checking the key and then indexing looks the key up twice; get does it once.",
            "\"k\" in d and d[\"k\"] becomes d.get(\"k\").",
        ),
    ),
    (
        PythonRule::FURB192,
        RuleText::new(
            "sorted() only to take the minimum or maximum",
            "use min() or max()",
            "Sorting the whole sequence to keep one element takes O(n log n) instead of O(n).",
            "sorted(values)[0] becomes min(values).",
        ),
    ),
    (
        PythonRule::PLR1716,
        RuleText::new(
            "Comparisons that can be chained",
            "chain them: a < b < c",
            "Python chains comparisons, which reads better and evaluates the middle value once.",
            "a < b and b < c becomes a < b < c.",
        ),
    ),
    (
        PythonRule::FURB105,
        RuleText::new(
            "Unnecessary print() argument",
            "remove the empty string or the sep",
            "print() already prints an empty line, and sep only matters between two or more values.",
            "print(\"\") becomes print().",
        ),
    ),
    (
        PythonRule::RUF057,
        RuleText::new(
            "round() that changes nothing",
            "remove the round() call",
            "Rounding an integer without negative digits returns the same integer.",
            "round(1, 0) becomes 1.",
        ),
    ),
    (
        PythonRule::PLE0605,
        RuleText::new(
            "__all__ that is not a list or tuple",
            "write __all__ as a list or tuple of names",
            "Tools and from module import * read __all__ as a sequence of names; a string is taken letter by letter.",
            "__all__ = \"Foo\" becomes __all__ = (\"Foo\",).",
        ),
    ),
    (
        PythonRule::PLW0131,
        RuleText::new(
            "Named expression used as a statement",
            "use a plain assignment",
            "The walrus only makes sense inside another expression; alone it is an assignment written in a harder way.",
            "(a := 42) becomes a = 42.",
        ),
    ),
    (
        PythonRule::RUF041,
        RuleText::new(
            "Nested Literal",
            "flatten it into a single Literal",
            "A Literal inside another Literal adds nothing: its values are already part of the outer one.",
            "Literal[Literal[\"a\"], \"b\"] becomes Literal[\"a\", \"b\"].",
        ),
    ),
    (
        PythonRule::D419,
        RuleText::new(
            "Empty docstring",
            "write the docstring or remove it",
            "An empty docstring promises documentation that is not there.",
            "\"\"\"\"\"\" becomes \"\"\"Return the mean of the values.\"\"\".",
        ),
    ),
    (
        PythonRule::FURB166,
        RuleText::new(
            "int() of a string with its prefix cut off",
            "use int(text, 0)",
            "With base 0, int() reads the 0b, 0o or 0x prefix itself, without slicing the string.",
            "int(text[2:], 16) becomes int(text, 0).",
        ),
    ),
    (
        PythonRule::RUF051,
        RuleText::new(
            "if key in dict before del",
            "use dict.pop(key, None)",
            "pop with a default removes the key in one step without a separate check.",
            "if k in d: del d[k] becomes d.pop(k, None).",
        ),
    ),
    (
        PythonRule::UP014,
        RuleText::new(
            "NamedTuple in functional syntax",
            "write it as a class",
            "The class syntax lets each field have its own annotation, default and docstring.",
            "Point = NamedTuple(\"Point\", [(\"x\", int)]) becomes class Point(NamedTuple): x: int.",
        ),
    ),
    (
        PythonRule::G201,
        RuleText::new(
            "logging.error with exc_info=True",
            "use logging.exception",
            "logging.exception logs at error level and attaches the traceback, with less to write.",
            "logging.error(\"failed\", exc_info=True) becomes logging.exception(\"failed\").",
        ),
    ),
    (
        PythonRule::B035,
        RuleText::new(
            "Dict comprehension with a static key",
            "use a key that depends on the loop",
            "A key that does not change keeps only the last value, so the comprehension builds a one-item dict.",
            "{\"key\": v for v in data} becomes {v: v for v in data}.",
        ),
    ),
    (
        PythonRule::B005,
        RuleText::new(
            "strip() with repeated characters",
            "use removeprefix or removesuffix",
            "strip removes any of the given characters, not the string as a whole, so .txt also eats letters of the name.",
            "name.rstrip(\".txt\") becomes name.removesuffix(\".txt\").",
        ),
    ),
    (
        PythonRule::PLW0245,
        RuleText::new(
            "super used without calling it",
            "write super()",
            "super without parentheses is the builtin class, not the parent of this object, so the method lookup fails.",
            "super.__init__() becomes super().__init__().",
        ),
    ),
    (
        PythonRule::B021,
        RuleText::new(
            "f-string used as a docstring",
            "write a plain string",
            "Python does not take an f-string as a docstring, so the object ends up without documentation.",
            "f\"\"\"Docs {x}\"\"\" becomes \"\"\"Docs.\"\"\".",
        ),
    ),
    (
        PythonRule::B012,
        RuleText::new(
            "return, break or continue inside finally",
            "move it out of the finally",
            "Leaving a finally this way silently discards the exception being raised.",
            "finally: return 1 becomes the return after the try.",
        ),
    ),
    (
        PythonRule::B013,
        RuleText::new(
            "One-element tuple in an except",
            "catch the exception without the tuple",
            "A tuple of one exception is the same as the exception alone.",
            "except (ValueError,): becomes except ValueError:.",
        ),
    ),
];
