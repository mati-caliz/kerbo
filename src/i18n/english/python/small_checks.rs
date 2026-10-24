use crate::i18n::catalog::RuleText;
use crate::rules::PythonRule;

// Más reglas del conjunto por defecto de ruff 0.17: métodos especiales, conversiones y constantes.
pub const SMALL_CHECK_TEXTS: [(PythonRule, RuleText); 30] = [
    (
        PythonRule::PLR1730,
        RuleText::new(
            "if statement that is min() or max()",
            "use min() or max()",
            "An if that keeps the lesser or greater of two values says the same as min() or max(), in more lines.",
            "if x > top: top = x becomes top = max(top, x).",
        ),
    ),
    (
        PythonRule::PERF102,
        RuleText::new(
            "dict.items() that discards the key or the value",
            "iterate dict.keys() or dict.values()",
            "Building each key and value pair only to throw half away is slower and hides what the loop needs.",
            "for _, value in d.items(): becomes for value in d.values():.",
        ),
    ),
    (
        PythonRule::C414,
        RuleText::new(
            "Unnecessary double cast",
            "remove the inner call",
            "The outer call already builds a new list, tuple, set or sorted copy, so the inner one is wasted work.",
            "list(tuple(items)) becomes list(items).",
        ),
    ),
    (
        PythonRule::C403,
        RuleText::new(
            "set() of a list comprehension",
            "write a set comprehension",
            "A set comprehension builds the set directly, without an intermediate list.",
            "set([x for x in items]) becomes {x for x in items}.",
        ),
    ),
    (
        PythonRule::C404,
        RuleText::new(
            "dict() of a list comprehension of pairs",
            "write a dict comprehension",
            "A dict comprehension builds the dictionary directly, without an intermediate list of tuples.",
            "dict([(k, v) for k, v in pairs]) becomes {k: v for k, v in pairs}.",
        ),
    ),
    (
        PythonRule::C411,
        RuleText::new(
            "list() of a list comprehension",
            "remove the list() call",
            "The comprehension already builds a new list.",
            "list([x for x in items]) becomes [x for x in items].",
        ),
    ),
    (
        PythonRule::C418,
        RuleText::new(
            "dict() of a dict literal",
            "remove the dict() call",
            "The literal or comprehension already builds a new dictionary.",
            "dict({\"a\": 1}) becomes {\"a\": 1}.",
        ),
    ),
    (
        PythonRule::PLW1508,
        RuleText::new(
            "os.getenv default that is not a string",
            "pass the default as a string",
            "os.getenv returns a string when the variable is set, so a default of another type makes the result inconsistent.",
            "os.getenv(\"PORT\", 80) becomes os.getenv(\"PORT\", \"80\").",
        ),
    ),
    (
        PythonRule::RUF007,
        RuleText::new(
            "zip() over successive pairs",
            "use itertools.pairwise",
            "itertools.pairwise says directly that the loop walks consecutive pairs.",
            "zip(items, items[1:]) becomes itertools.pairwise(items).",
        ),
    ),
    (
        PythonRule::PLC0105,
        RuleText::new(
            "TypeVar name that does not match its variance",
            "add _co or _contra, or remove it",
            "PEP 484 names covariant type variables with _co and contravariant ones with _contra, so readers know the variance from the name.",
            "T = TypeVar(\"T\", covariant=True) becomes T_co = TypeVar(\"T_co\", covariant=True).",
        ),
    ),
    (
        PythonRule::SIM222,
        RuleText::new(
            "or with an always true operand",
            "remove what can never run",
            "The or stops at the first true value, so everything after the constant never runs.",
            "x or True or y becomes x or True.",
        ),
    ),
    (
        PythonRule::SIM223,
        RuleText::new(
            "and with an always false operand",
            "remove what can never run",
            "The and stops at the first false value, so everything after the constant never runs.",
            "x and False and y becomes x and False.",
        ),
    ),
    (
        PythonRule::SIM208,
        RuleText::new(
            "Double negation",
            "remove both nots, or use bool()",
            "not not x is the same as bool(x), and just x in a condition.",
            "if not not x: becomes if x:.",
        ),
    ),
    (
        PythonRule::SIM211,
        RuleText::new(
            "False if x else True",
            "write not x",
            "The ternary only negates the condition.",
            "False if x else True becomes not x.",
        ),
    ),
    (
        PythonRule::RUF034,
        RuleText::new(
            "Ternary with the same value in both branches",
            "keep just the value",
            "Both branches give the same result, so the condition does nothing.",
            "\"a\" if x else \"a\" becomes \"a\".",
        ),
    ),
    (
        PythonRule::B030,
        RuleText::new(
            "except with something that is not an exception class",
            "catch exception classes",
            "Catching a value that is not a class derived from BaseException raises TypeError when an exception arrives.",
            "except 1: becomes except ValueError:.",
        ),
    ),
    (
        PythonRule::B016,
        RuleText::new(
            "raise of a literal",
            "raise an exception instance",
            "Raising a literal is a TypeError: only exceptions can be raised.",
            "raise \"boom\" becomes raise RuntimeError(\"boom\").",
        ),
    ),
    (
        PythonRule::PLW0129,
        RuleText::new(
            "assert on a string literal",
            "assert a real condition",
            "A non-empty string is always true and an empty one always false, so the assert never checks anything.",
            "assert \"ready\" becomes assert ready, \"not ready\".",
        ),
    ),
    (
        PythonRule::PLW0177,
        RuleText::new(
            "Comparison with NaN",
            "use math.isnan",
            "NaN is not equal to anything, not even to itself, so the comparison always gives the same result.",
            "x == float(\"nan\") becomes math.isnan(x).",
        ),
    ),
    (
        PythonRule::PLE0101,
        RuleText::new(
            "__init__ that returns a value",
            "remove the returned value",
            "__init__ must return None; returning anything else raises TypeError when the object is created.",
            "return self.value inside __init__ becomes return.",
        ),
    ),
    (
        PythonRule::PIE800,
        RuleText::new(
            "Unnecessary ** spread of a dict literal",
            "write the keys directly",
            "Spreading a literal dictionary into another one only adds noise.",
            "{**base, **{\"a\": 1}} becomes {**base, \"a\": 1}.",
        ),
    ),
    (
        PythonRule::PLE0303,
        RuleText::new(
            "__len__ that does not return a non-negative int",
            "return a non-negative integer",
            "len() raises TypeError or ValueError when __len__ returns anything else.",
            "return \"2\" inside __len__ becomes return 2.",
        ),
    ),
    (
        PythonRule::PLE0304,
        RuleText::new(
            "__bool__ that does not return a bool",
            "return True or False",
            "bool() raises TypeError when __bool__ returns anything other than a bool.",
            "return 1 inside __bool__ becomes return True.",
        ),
    ),
    (
        PythonRule::PLE0305,
        RuleText::new(
            "__index__ that does not return an int",
            "return an integer",
            "Python raises TypeError when __index__ returns anything other than an int.",
            "return \"1\" inside __index__ becomes return 1.",
        ),
    ),
    (
        PythonRule::PLE0307,
        RuleText::new(
            "__str__ that does not return a str",
            "return a string",
            "str() raises TypeError when __str__ returns anything other than a str.",
            "return 3 inside __str__ becomes return \"3\".",
        ),
    ),
    (
        PythonRule::PLE0308,
        RuleText::new(
            "__bytes__ that does not return bytes",
            "return a bytes object",
            "bytes() raises TypeError when __bytes__ returns anything other than bytes.",
            "return \"text\" inside __bytes__ becomes return b\"text\".",
        ),
    ),
    (
        PythonRule::PLE0309,
        RuleText::new(
            "__hash__ that does not return an int",
            "return an integer",
            "hash() raises TypeError when __hash__ returns anything other than an int.",
            "return 1.5 inside __hash__ becomes return hash(self.value).",
        ),
    ),
    (
        PythonRule::PLR0206,
        RuleText::new(
            "Property that takes parameters",
            "turn it into a regular method",
            "A property is read without calling it, so its extra parameters can never receive a value.",
            "@property def size(self, scale) becomes def size(self, scale).",
        ),
    ),
    (
        PythonRule::UP019,
        RuleText::new(
            "typing.Text instead of str",
            "use str",
            "typing.Text is an alias of str kept for Python 2 and is deprecated.",
            "name: Text becomes name: str.",
        ),
    ),
    (
        PythonRule::PIE796,
        RuleText::new(
            "Repeated value in an Enum",
            "give each member its own value",
            "A repeated value turns the member into an alias of the first one instead of a new member.",
            "BLUE = 1 after RED = 1 becomes BLUE = 3.",
        ),
    ),
];
