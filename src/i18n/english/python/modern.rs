use crate::i18n::catalog::RuleText;
use crate::rules::PythonRule;

// Las reglas del conjunto por defecto de ruff 0.17 fuera del clásico (pyupgrade, flake8-comprehensions
// y compañía).
pub const MODERN_TEXTS: [(PythonRule, RuleText); 35] = [
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
    (
        PythonRule::SIM102,
        RuleText::new(
            "An if whose only content is another if",
            "join both conditions with and",
            "Two nested ifs with nothing else add an indentation level for nothing: a single condition reads better.",
            "if a:\n    if b: becomes if a and b:.",
        ),
    ),
    (
        PythonRule::SIM117,
        RuleText::new(
            "A with whose only content is another with",
            "open both contexts in the same with",
            "One with per context adds indentation for nothing: with a, b: says the same.",
            "with open(x) as a:\n    with open(y) as b: becomes with open(x) as a, open(y) as b:.",
        ),
    ),
    (
        PythonRule::BLE001,
        RuleText::new(
            "An except that catches Exception without re-raising or logging it",
            "catch the specific exception you expect, or log it with logging.exception",
            "Blindly catching Exception hides programming errors and unexpected failures: the program carries on as if nothing happened.",
            "except Exception: pass becomes except ValueError: pass, or except Exception: logger.exception(\"failed\").",
        ),
    ),
    (
        PythonRule::TRY002,
        RuleText::new(
            "Exception or BaseException is raised directly",
            "define your own exception, or use a more specific one",
            "Whoever catches the error can't tell it apart from any other: all they can do is catch the whole Exception.",
            "raise Exception(\"no funds\") becomes raise InsufficientFunds(\"no funds\").",
        ),
    ),
    (
        PythonRule::PLC0414,
        RuleText::new(
            "An import with an alias equal to the original name",
            "drop the as",
            "import x as x says the same as import x; it only makes sense in __init__.py and in stubs, to re-export.",
            "import os as os becomes import os.",
        ),
    ),
    (
        PythonRule::S102,
        RuleText::new(
            "Use of exec",
            "avoid exec: solve the problem without running code built at run time",
            "exec runs any code it gets: if the text comes from outside, it's an open door.",
            "exec(code) becomes a function or dispatch table that does the same.",
        ),
    ),
    (
        PythonRule::S110,
        RuleText::new(
            "An except that catches everything and only does pass",
            "log the error, or catch only the exception you expect",
            "An error swallowed in silence leaves no trace: later nobody knows why the program did something else.",
            "except Exception: pass becomes except Exception: logger.exception(\"failed\").",
        ),
    ),
    (
        PythonRule::SIM118,
        RuleText::new(
            "A key is looked up in d.keys()",
            "look it up in the dictionary directly",
            "key in d does the same as key in d.keys() without creating an intermediate view, and reads better.",
            "if key in d.keys(): becomes if key in d:.",
        ),
    ),
    (
        PythonRule::UP017,
        RuleText::new(
            "Use of datetime.timezone.utc",
            "use datetime.UTC",
            "Since Python 3.11 datetime.UTC is the short alias for the UTC time zone.",
            "datetime.timezone.utc becomes datetime.UTC.",
        ),
    ),
    (
        PythonRule::UP018,
        RuleText::new(
            "A str, int, float, bool, bytes or complex that wraps a literal of the same type",
            "write the literal directly",
            "The call doesn't change the value: it only adds noise and an extra call.",
            "str(\"hello\") becomes \"hello\", and int() becomes 0.",
        ),
    ),
    (
        PythonRule::UP030,
        RuleText::new(
            "A .format with numbered positions in order",
            "drop the numbers from the braces",
            "\"{0} {1}\" says the same as \"{} {}\": the numbers only add something to maintain.",
            "\"{0} {1}\".format(a, b) becomes \"{} {}\".format(a, b).",
        ),
    ),
    (
        PythonRule::UP008,
        RuleText::new(
            "super with the class and the instance as arguments",
            "use super() without arguments",
            "Since Python 3, super() finds the class and the instance by itself; repeating them adds something that breaks when the class is renamed.",
            "super(Child, self).__init__() becomes super().__init__().",
        ),
    ),
    (
        PythonRule::PIE808,
        RuleText::new(
            "range with an explicit 0 as start",
            "drop the 0: range already starts there",
            "range(0, n) says the same as range(n) with more noise.",
            "range(0, 10) becomes range(10).",
        ),
    ),
    (
        PythonRule::B009,
        RuleText::new(
            "getattr with a fixed attribute name",
            "access the attribute directly",
            "getattr(x, \"a\") is x.a written indirectly: the analyzer and the editor lose track of the attribute.",
            "getattr(user, \"name\") becomes user.name.",
        ),
    ),
    (
        PythonRule::B010,
        RuleText::new(
            "setattr with a fixed attribute name",
            "assign the attribute directly",
            "setattr(x, \"a\", v) is x.a = v written indirectly: the analyzer and the editor lose track of the attribute.",
            "setattr(user, \"name\", value) becomes user.name = value.",
        ),
    ),
    (
        PythonRule::RUF015,
        RuleText::new(
            "A whole list or tuple is built just to take the first element",
            "use next(iter(...))",
            "list(x)[0] walks and copies the whole iterable to keep one element.",
            "list(users)[0] becomes next(iter(users)).",
        ),
    ),
    (
        PythonRule::B006,
        RuleText::new(
            "A parameter with a mutable default value",
            "use None as default and create the value inside the function",
            "The default is created once, when the function is defined: every call shares and modifies the same object.",
            "def add(x, items=[]): becomes def add(x, items=None): with items = [] when None.",
        ),
    ),
    (
        PythonRule::B008,
        RuleText::new(
            "A call in a parameter's default value",
            "compute the value inside the function, or keep it in a module constant",
            "The default is evaluated once, when the function is defined, not on each call: a now() stays frozen.",
            "def log(moment=datetime.now()): becomes def log(moment=None): with moment = datetime.now() when None.",
        ),
    ),
    (
        PythonRule::RUF012,
        RuleText::new(
            "A class attribute with a mutable value",
            "annotate it with ClassVar if it's shared on purpose, or create it in __init__",
            "The value is created once and shared by every instance: modifying it from one changes it for all.",
            "items = [] becomes items: ClassVar[list[str]] = [], or self.items = [] in __init__.",
        ),
    ),
    (
        PythonRule::RUF059,
        RuleText::new(
            "An unpacked variable that is never used",
            "prefix it with _, or replace it with _",
            "A name that is unpacked and never read suggests the value matters, and hides typos.",
            "name, age = person with age unused becomes name, _age = person.",
        ),
    ),
    (
        PythonRule::SIM115,
        RuleText::new(
            "A file is opened outside a with",
            "open it with with",
            "Without with, the file stays open if something fails before closing it, and relies on the garbage collector.",
            "f = open(path) becomes with open(path) as f:.",
        ),
    ),
    (
        PythonRule::TRY004,
        RuleText::new(
            "A type check that raises an exception other than TypeError",
            "raise TypeError",
            "If the problem is the value's type, TypeError is what the caller expects; a generic exception confuses them.",
            "if not isinstance(x, int): raise ValueError(...) becomes raise TypeError(...).",
        ),
    ),
];
