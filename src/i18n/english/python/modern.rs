use crate::i18n::catalog::RuleText;
use crate::rules::PythonRule;

// Las reglas del conjunto por defecto de ruff 0.17 fuera del clásico (pyupgrade, flake8-comprehensions
// y compañía).
pub const MODERN_TEXTS: [(PythonRule, RuleText); 13] = [
    (
        PythonRule::C408,
        RuleText::new(
            "dict(), list() or tuple() instead of the literal",
            "use {}, [] or ()",
            "The call is slower than the literal and can be shadowed by another dict or list; the literal says the same without surprises.",
            "dict(a=1) becomes {\"a\": 1} and list() becomes [].",
        ),
    ),
    (
        PythonRule::UP031,
        RuleText::new(
            "% formatting instead of str.format or an f-string",
            "use an f-string or str.format",
            "% formatting has odd cases (a tuple as the only value, mismatched types) and reads worse than an f-string.",
            "\"hello %s\" % name becomes f\"hello {name}\".",
        ),
    ),
    (
        PythonRule::UP004,
        RuleText::new(
            "A class that inherits from object",
            "remove object from the bases",
            "In Python 3 every class inherits from object: writing it adds nothing.",
            "class Order(object): becomes class Order:.",
        ),
    ),
    (
        PythonRule::UP005,
        RuleText::new(
            "A unittest alias removed in Python 3.12",
            "use the new name",
            "assertEquals and friends were old aliases: they no longer exist in 3.12 and the test fails.",
            "self.assertEquals(a, b) becomes self.assertEqual(a, b).",
        ),
    ),
    (
        PythonRule::UP009,
        RuleText::new(
            "A utf-8 encoding declaration",
            "delete it",
            "In Python 3 source code is already utf-8 by default: the line is redundant.",
            "# -*- coding: utf-8 -*- is deleted.",
        ),
    ),
    (
        PythonRule::UP025,
        RuleText::new(
            "A u prefix on a string",
            "drop the u",
            "In Python 3 every string is unicode: the u prefix changes nothing.",
            "u\"hello\" becomes \"hello\".",
        ),
    ),
    (
        PythonRule::C405,
        RuleText::new(
            "set() with a list or tuple literal",
            "use a set literal",
            "Building a list just to turn it into a set is slower than writing the set directly.",
            "set([1, 2]) becomes {1, 2}.",
        ),
    ),
    (
        PythonRule::B015,
        RuleText::new(
            "A comparison whose result is discarded",
            "use it in an assert or an if, or delete it",
            "A bare comparison does nothing: the assert or the if is almost always missing.",
            "x == 1 becomes assert x == 1.",
        ),
    ),
    (
        PythonRule::B018,
        RuleText::new(
            "A bare expression that does nothing",
            "delete it or use its value",
            "An expression without effects as a statement is computed and thrown away: usually leftover code or a bug.",
            "self.loaded is deleted or becomes return self.loaded.",
        ),
    ),
    (
        PythonRule::PIE790,
        RuleText::new(
            "An unnecessary pass or ...",
            "delete it",
            "The block already has another statement: the pass or ... is not needed.",
            "def f():\n    \"\"\"Doc.\"\"\"\n    pass loses the pass.",
        ),
    ),
    (
        PythonRule::UP006,
        RuleText::new(
            "List, Dict and friends from typing where list, dict will do",
            "use the builtin or collections type",
            "Since Python 3.9 list[int] works the same: the typing import is redundant and reads worse.",
            "List[int] becomes list[int] and typing.Dict[str, int] becomes dict[str, int].",
        ),
    ),
    (
        PythonRule::UP007,
        RuleText::new(
            "Union[X, Y] instead of X | Y",
            "write the union with |",
            "Since Python 3.10 the | union is shorter and needs no import.",
            "Union[int, str] becomes int | str.",
        ),
    ),
    (
        PythonRule::UP045,
        RuleText::new(
            "Optional[X] instead of X | None",
            "write X | None",
            "Since Python 3.10 X | None says the same without importing Optional.",
            "Optional[int] becomes int | None.",
        ),
    ),
];
