use crate::i18n::catalog::RuleText;
use crate::rules::PythonRule;

// Textos de las reglas de llamadas a bibliotecas y excepciones del conjunto por defecto de ruff 0.17 que no aparecen en el corpus.
pub const LIBRARY_RULE_TEXTS: [(PythonRule, RuleText); 22] = [
    (
        PythonRule::ASYNC105,
        RuleText::new(
            "trio async function called without await",
            "await the call",
            "Without await the coroutine is created and thrown away, so the function never runs.",
            "trio.sleep(1) becomes await trio.sleep(1).",
        ),
    ),
    (
        PythonRule::ASYNC116,
        RuleText::new(
            "sleep of more than a day in trio or anyio",
            "use sleep_forever",
            "A sleep that long is almost always meant to wait forever, and sleep_forever says so.",
            "await trio.sleep(10**10) becomes await trio.sleep_forever().",
        ),
    ),
    (
        PythonRule::ASYNC210,
        RuleText::new(
            "Blocking HTTP call inside an async function",
            "use an async HTTP client",
            "A blocking request stops the whole event loop until the response arrives.",
            "requests.get(url) becomes await client.get(url) with httpx.AsyncClient.",
        ),
    ),
    (
        PythonRule::ASYNC220,
        RuleText::new(
            "Blocking subprocess creation inside an async function",
            "use asyncio.create_subprocess_exec",
            "Creating the process with the blocking API stops the event loop while it starts.",
            "subprocess.Popen(cmd) becomes await asyncio.create_subprocess_exec(*cmd).",
        ),
    ),
    (
        PythonRule::ASYNC221,
        RuleText::new(
            "Blocking process run inside an async function",
            "run the process with the async subprocess API",
            "Waiting for the process with the blocking API stops the event loop until it ends.",
            "subprocess.run(cmd) becomes await (await asyncio.create_subprocess_exec(*cmd)).wait().",
        ),
    ),
    (
        PythonRule::B014,
        RuleText::new(
            "Exception repeated in an except tuple",
            "list each exception once",
            "The repeated name adds nothing and usually hides a typo for another exception.",
            "except (ValueError, ValueError): becomes except ValueError:.",
        ),
    ),
    (
        PythonRule::B022,
        RuleText::new(
            "contextlib.suppress without exceptions",
            "pass the exceptions to suppress, or remove it",
            "Without arguments suppress catches nothing, so the with block only adds noise.",
            "with contextlib.suppress(): becomes with contextlib.suppress(FileNotFoundError):.",
        ),
    ),
    (
        PythonRule::B025,
        RuleText::new(
            "Exception already caught by an earlier except",
            "remove it from this handler",
            "The first handler that matches wins, so this one never runs for that exception.",
            "except ValueError: ... except ValueError: leaves only the first.",
        ),
    ),
    (
        PythonRule::B029,
        RuleText::new(
            "except with an empty tuple",
            "list the exceptions to catch",
            "except (): catches nothing, so the handler never runs.",
            "except (): becomes except (ValueError, TypeError):.",
        ),
    ),
    (
        PythonRule::B039,
        RuleText::new(
            "Mutable default for a ContextVar",
            "use None as the default and create the value when it is missing",
            "The default object is shared by every context, so changing it leaks between them.",
            "ContextVar(\"items\", default=[]) becomes ContextVar(\"items\", default=None).",
        ),
    ),
    (
        PythonRule::C415,
        RuleText::new(
            "Reversal of an iterable that is sorted, reversed or turned into a set",
            "remove the [::-1]",
            "sorted, set and reversed do not depend on the original order in a way the reversal helps.",
            "sorted(values[::-1]) becomes sorted(values).",
        ),
    ),
    (
        PythonRule::SIM107,
        RuleText::new(
            "return in finally that overrides an earlier return",
            "move the return out of the finally",
            "A return in finally always wins, so the value returned from try or except is lost.",
            "try: return 1 finally: return 2 always returns 2.",
        ),
    ),
    (
        PythonRule::SIM220,
        RuleText::new(
            "and between a name and its negation",
            "replace the expression with False",
            "x and not x is always false, which usually means the condition is wrong.",
            "if a and not a: never runs.",
        ),
    ),
    (
        PythonRule::SIM221,
        RuleText::new(
            "or between a name and its negation",
            "replace the expression with True",
            "x or not x is always true, which usually means the condition is wrong.",
            "if a or not a: always runs.",
        ),
    ),
    (
        PythonRule::SIM911,
        RuleText::new(
            "zip of a dict's keys and values",
            "use dict.items()",
            "items() already yields the pairs, without building two views and zipping them.",
            "zip(d.keys(), d.values()) becomes d.items().",
        ),
    ),
    (
        PythonRule::FURB162,
        RuleText::new(
            "Z replaced by hand before datetime.fromisoformat",
            "pass the string directly",
            "Since Python 3.11 fromisoformat understands the Z suffix.",
            "datetime.fromisoformat(s.replace(\"Z\", \"+00:00\")) becomes datetime.fromisoformat(s).",
        ),
    ),
    (
        PythonRule::FURB169,
        RuleText::new(
            "type(x) compared with type(None)",
            "use x is None",
            "Comparing types to check for None is a roundabout way to write an identity check.",
            "type(x) == type(None) becomes x is None.",
        ),
    ),
    (
        PythonRule::FURB177,
        RuleText::new(
            "Path().resolve() to get the current directory",
            "use Path.cwd()",
            "Path.cwd() says what it does and does not depend on how an empty path resolves.",
            "Path().resolve() becomes Path.cwd().",
        ),
    ),
    (
        PythonRule::FURB181,
        RuleText::new(
            "hashlib digest().hex()",
            "use hexdigest()",
            "hexdigest() returns the same text directly, without building the bytes first.",
            "hashlib.sha256(data).digest().hex() becomes hashlib.sha256(data).hexdigest().",
        ),
    ),
    (
        PythonRule::PTH210,
        RuleText::new(
            "with_suffix with a suffix that does not start with a dot",
            "add the leading dot",
            "with_suffix raises ValueError for a suffix without a dot, or a lone dot.",
            "path.with_suffix(\"txt\") becomes path.with_suffix(\".txt\").",
        ),
    ),
    (
        PythonRule::PGH005,
        RuleText::new(
            "Mock method not called, or that does not exist",
            "call the assert method, or use the right name",
            "mock.assert_called is truthy without being called, and mock.called_once is created on the fly, so the check always passes.",
            "mock.assert_called_once becomes mock.assert_called_once().",
        ),
    ),
    (
        PythonRule::PLC0131,
        RuleText::new(
            "TypeVar that is both covariant and contravariant",
            "keep only one of the variances",
            "A type variable cannot be both at once; Python raises ValueError when it is created.",
            "TypeVar(\"T\", covariant=True, contravariant=True) keeps only covariant=True.",
        ),
    ),
];
