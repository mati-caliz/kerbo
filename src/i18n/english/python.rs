mod formats;
mod modern;

use crate::i18n::catalog::RuleText;
use crate::rules::PythonRule;
use formats::FORMAT_TEXTS;
use modern::MODERN_TEXTS;

const SYNTAX_WHY: &str = "Python rejects it when compiling the module: the file cannot even be imported.";
const ALWAYS_TRUE_WHY: &str = "A non-empty tuple is always truthy: the condition depends on nothing.";
const AMBIGUOUS_WHY: &str = "In many fonts l, I and O look like 1 and 0: the code is easy to misread.";

const UNKNOWN: RuleText = RuleText::new(
    "A ruff rule",
    "follow the rule",
    "ruff reports it in its default set.",
    "See the ruff documentation.",
);
const TEXTS: [(PythonRule, RuleText); 41] = [
    (
        PythonRule::F401,
        RuleText::new(
            "Unused import",
            "delete the import, or re-export it with __all__ or `import x as x`",
            "An import nobody uses still loads the module, adds fake dependencies and hides the ones that matter.",
            "import os with no os. anywhere gets deleted.",
        ),
    ),
    (
        PythonRule::F402,
        RuleText::new(
            "The loop variable shadows an import",
            "rename the loop variable",
            "After the loop the name is no longer the module: any later use fails in a confusing way.",
            "import path and for path in paths: becomes for item in paths:.",
        ),
    ),
    (
        PythonRule::F403,
        RuleText::new(
            "Wildcard import",
            "import the names you use explicitly",
            "With import * nobody knows where each name comes from and no check can catch the ones that do not exist.",
            "from os.path import * becomes from os.path import join, exists.",
        ),
    ),
    (
        PythonRule::F404,
        RuleText::new(
            "from __future__ after other code",
            "move the __future__ import to the top of the file",
            SYNTAX_WHY,
            "from __future__ import annotations goes before any other import.",
        ),
    ),
    (
        PythonRule::F405,
        RuleText::new(
            "A name that may be undefined or come from a wildcard import",
            "import the name explicitly",
            "With an import * in the module there is no way to tell whether the name exists: if not, it fails only at run time.",
            "from os.path import * and join(a, b) becomes from os.path import join.",
        ),
    ),
    (
        PythonRule::F406,
        RuleText::new(
            "Wildcard import outside the module level",
            "import the names explicitly",
            SYNTAX_WHY,
            "def f(): from os import * becomes def f(): from os import path.",
        ),
    ),
    (
        PythonRule::F541,
        RuleText::new(
            "f-string without any placeholder",
            "drop the f",
            "An f-string without braces is a plain string that suggests something was meant to be interpolated.",
            "f\"hello\" becomes \"hello\".",
        ),
    ),
    (
        PythonRule::F601,
        RuleText::new(
            "Literal key repeated in a dictionary",
            "keep a single entry per key",
            "The last one silently overwrites the earlier ones: the first value is lost.",
            "{\"a\": 1, \"a\": 2} is {\"a\": 2}.",
        ),
    ),
    (
        PythonRule::F602,
        RuleText::new(
            "Variable repeated as a dictionary key",
            "keep a single entry per key",
            "If both are the same variable, the last one silently overwrites the first.",
            "{key: 1, key: 2} is {key: 2}.",
        ),
    ),
    (
        PythonRule::F631,
        RuleText::new(
            "assert on a tuple",
            "drop the parentheses: assert condition, \"message\"",
            ALWAYS_TRUE_WHY,
            "assert (x > 0, \"positive\") becomes assert x > 0, \"positive\".",
        ),
    ),
    (
        PythonRule::F632,
        RuleText::new(
            "is against a literal",
            "compare with == or !=",
            "is compares identity: with numbers or strings it depends on interpreter details and may be False.",
            "x is \"a\" becomes x == \"a\".",
        ),
    ),
    (
        PythonRule::F633,
        RuleText::new(
            "Python 2 print >>",
            "use print(..., file=target)",
            "In Python 3 print is a function: >> does not redirect, it shifts bits and fails.",
            "print >> sys.stderr, \"x\" becomes print(\"x\", file=sys.stderr).",
        ),
    ),
    (
        PythonRule::F634,
        RuleText::new(
            "if on a tuple",
            "remove the comma or the parentheses that build the tuple",
            ALWAYS_TRUE_WHY,
            "if (a, b): becomes if a and b:.",
        ),
    ),
    (
        PythonRule::F701,
        RuleText::new(
            "break outside a loop",
            "remove the break or move it inside the for or while",
            SYNTAX_WHY,
            "A break in the else of a for breaks the outer loop, not that one.",
        ),
    ),
    (
        PythonRule::F702,
        RuleText::new(
            "continue outside a loop",
            "remove the continue or move it inside the for or while",
            SYNTAX_WHY,
            "A continue in a function body that is not inside a loop does not compile.",
        ),
    ),
    (
        PythonRule::F704,
        RuleText::new(
            "yield or await outside a function",
            "move it inside a function (async, for await)",
            SYNTAX_WHY,
            "A module-level await goes inside an async def run with asyncio.run.",
        ),
    ),
    (
        PythonRule::F706,
        RuleText::new(
            "return outside a function",
            "remove the return or move it inside a function",
            SYNTAX_WHY,
            "A return in a class body does not compile.",
        ),
    ),
    (
        PythonRule::F707,
        RuleText::new(
            "Bare except: before other except clauses",
            "leave the bare except last",
            SYNTAX_WHY,
            "except: followed by except ValueError: becomes except ValueError: and finally except Exception:.",
        ),
    ),
    (
        PythonRule::F722,
        RuleText::new(
            "A quoted annotation that is not valid Python",
            "fix the annotation syntax",
            "Type checkers cannot read it and typing.get_type_hints fails when evaluating it.",
            "x: \"List[int\" becomes x: \"List[int]\".",
        ),
    ),
    (
        PythonRule::F811,
        RuleText::new(
            "Redefinition of something never used",
            "delete the earlier definition or rename one of them",
            "The first definition is overwritten without being used: almost always a duplicated import or function.",
            "Two def parse(...) in a row: the first is never called.",
        ),
    ),
    (
        PythonRule::F821,
        RuleText::new(
            "Undefined name",
            "define or import the name, or fix the typo",
            "The code fails with NameError only when it reaches that line, in production if no test goes through it.",
            "print(counter) with no counter defined before.",
        ),
    ),
    (
        PythonRule::F822,
        RuleText::new(
            "__all__ exports a name that does not exist",
            "define the name or remove it from __all__",
            "from module import * fails with AttributeError when looking that name up.",
            "__all__ = [\"parse\"] with no parse in the module.",
        ),
    ),
    (
        PythonRule::F823,
        RuleText::new(
            "Local variable used before assignment",
            "assign it before using it, or declare it global or nonlocal",
            "Since the function assigns it somewhere, Python treats it as local in the whole function: the earlier use fails with UnboundLocalError.",
            "x = 1 and def f(): print(x); x = 2 fails at the print.",
        ),
    ),
    (
        PythonRule::F841,
        RuleText::new(
            "Local variable assigned and never used",
            "delete the assignment or use the variable",
            "An assignment nobody reads is dead code or a bug: maybe it was meant to be used somewhere else.",
            "except ValueError as error: pass becomes except ValueError: pass.",
        ),
    ),
    (
        PythonRule::F842,
        RuleText::new(
            "Local variable annotated and never used",
            "delete the annotation",
            "Declaring the type of something never assigned nor read is useless.",
            "total: int with no use of total in the function.",
        ),
    ),
    (
        PythonRule::F901,
        RuleText::new(
            "raise NotImplemented instead of NotImplementedError",
            "raise NotImplementedError",
            "NotImplemented is a value for operators, not an exception: the raise fails with TypeError.",
            "raise NotImplemented becomes raise NotImplementedError.",
        ),
    ),
    (
        PythonRule::E401,
        RuleText::new(
            "Several modules in one import",
            "one import per line",
            "Each import on its own line reads, sorts and gets deleted without touching the others.",
            "import os, sys becomes import os and import sys.",
        ),
    ),
    (
        PythonRule::E402,
        RuleText::new(
            "Module import not at the top of the file",
            "move the import up, before the rest of the code",
            "Imports together at the top show what the module depends on; one hidden further down gets lost.",
            "An import json after a function moves to the import list at the top.",
        ),
    ),
    (
        PythonRule::E701,
        RuleText::new(
            "Several statements on one line after a colon",
            "move the body to the next line, indented",
            "A body on the same line as the if or the for is easy to miss and makes diffs harder.",
            "if x: return becomes if x: and return on the next line.",
        ),
    ),
    (
        PythonRule::E702,
        RuleText::new(
            "Several statements on one line separated by semicolons",
            "one statement per line",
            "With ; in between it is hard to see where each statement ends.",
            "a = 1; b = 2 becomes two lines.",
        ),
    ),
    (
        PythonRule::E703,
        RuleText::new(
            "Semicolon at the end of the statement",
            "remove the semicolon",
            "In Python the trailing ; does nothing: it is noise brought from another language.",
            "x = 1; becomes x = 1.",
        ),
    ),
    (
        PythonRule::E711,
        RuleText::new(
            "Comparison to None with == or !=",
            "use is None or is not None",
            "== can be overridden and return anything; None is unique and is compared by identity.",
            "if x == None: becomes if x is None:.",
        ),
    ),
    (
        PythonRule::E712,
        RuleText::new(
            "Comparison to True or False with == or !=",
            "use the condition directly, or is when it must be exactly a boolean",
            "x == True is longer than x and does not do what it seems with values that are not booleans.",
            "if done == True: becomes if done:.",
        ),
    ),
    (
        PythonRule::E713,
        RuleText::new(
            "not x in y instead of x not in y",
            "use not in",
            "not in reads the way it is said and does not depend on the precedence of not.",
            "if not x in y: becomes if x not in y:.",
        ),
    ),
    (
        PythonRule::E714,
        RuleText::new(
            "not x is y instead of x is not y",
            "use is not",
            "is not reads the way it is said and does not depend on the precedence of not.",
            "if not x is None: becomes if x is not None:.",
        ),
    ),
    (
        PythonRule::E721,
        RuleText::new(
            "Type comparison with == or !=",
            "use isinstance() or, when it must be exactly that type, is",
            "== between types ignores subclasses and can be overridden in the metaclass.",
            "type(x) == int becomes isinstance(x, int) or type(x) is int.",
        ),
    ),
    (
        PythonRule::E722,
        RuleText::new(
            "Bare except:",
            "catch the exceptions you expect, or except Exception if it really is all of them",
            "A bare except also catches KeyboardInterrupt and SystemExit: the program cannot be stopped and errors are swallowed.",
            "except: becomes except (ValueError, KeyError):.",
        ),
    ),
    (
        PythonRule::E731,
        RuleText::new(
            "Lambda assigned to a variable",
            "write it as a def",
            "A def has a name in tracebacks and allows a docstring and annotations; the assigned lambda gains nothing.",
            "double = lambda x: x * 2 becomes def double(x): return x * 2.",
        ),
    ),
    (
        PythonRule::E741,
        RuleText::new(
            "Variable with an ambiguous name (l, I or O)",
            "use a descriptive name",
            AMBIGUOUS_WHY,
            "l = len(items) becomes count = len(items).",
        ),
    ),
    (
        PythonRule::E742,
        RuleText::new(
            "Class with an ambiguous name (l, I or O)",
            "use a descriptive name",
            AMBIGUOUS_WHY,
            "class I: becomes class Identity:.",
        ),
    ),
    (
        PythonRule::E743,
        RuleText::new(
            "Function with an ambiguous name (l, I or O)",
            "use a descriptive name",
            AMBIGUOUS_WHY,
            "def l(): becomes def lines():.",
        ),
    ),
];

pub fn python(rule: PythonRule) -> RuleText {
    TEXTS
        .iter()
        .chain(FORMAT_TEXTS.iter())
        .chain(MODERN_TEXTS.iter())
        .find(|(candidate, _)| *candidate == rule)
        .map_or(UNKNOWN, |(_, text)| *text)
}
