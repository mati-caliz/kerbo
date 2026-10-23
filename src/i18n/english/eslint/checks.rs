use crate::i18n::catalog::RuleText;
use crate::rules::EslintRule;

pub const CHECK_TEXTS: [(EslintRule, RuleText); 29] = [
    (
        EslintRule::NoConstAssign,
        RuleText::new(
            "Something is assigned to a constant",
            "use let if the value has to change, or store it in another variable",
            "Assigning to a const throws a TypeError when that line runs: the bug stays hidden until then.",
            "const total = 0; total += 1; becomes let total = 0; total += 1;.",
        ),
    ),
    (
        EslintRule::NoClassAssign,
        RuleText::new(
            "Something is assigned to a class name",
            "store the value in another variable",
            "After the assignment the name is no longer the class: whoever instantiates it further down gets something else.",
            "class Order {} Order = other; becomes const AlternativeOrder = other;.",
        ),
    ),
    (
        EslintRule::NoImportAssign,
        RuleText::new(
            "Something is assigned to an import",
            "copy the value to a local variable",
            "Imports are read-only: assigning to one throws, and so does mutating an imported namespace.",
            "import { limit } from './config'; limit = 10; becomes let localLimit = limit; localLimit = 10;.",
        ),
    ),
    (
        EslintRule::NoGlobalAssign,
        RuleText::new(
            "Something is assigned to a JavaScript global",
            "use another name",
            "Overwriting undefined, Object or Math breaks every piece of code that relies on them, and in strict mode it throws.",
            "undefined = 1; or Object = {}; is deleted or uses another name.",
        ),
    ),
    (
        EslintRule::NoShadowRestrictedNames,
        RuleText::new(
            "A declaration named after a restricted global",
            "pick another name",
            "Shadowing undefined, NaN, Infinity, arguments, eval or globalThis confuses the reader and can change what the code does.",
            "function f(undefined) {} becomes function f(value) {}.",
        ),
    ),
    (
        EslintRule::NoUnassignedVars,
        RuleText::new(
            "A variable that is read but never gets a value",
            "assign it a value or delete it",
            "It is always undefined: the assignment is almost always missing or the code is a leftover.",
            "let total; return total; becomes let total = compute(); return total;.",
        ),
    ),
    (
        EslintRule::ValidTypeof,
        RuleText::new(
            "typeof compared with a type that does not exist",
            "use one of the types typeof returns: string, number, object, etc.",
            "typeof never returns anything else: the comparison always gives the same result and is almost always a typo.",
            "typeof value === 'strng' becomes typeof value === 'string'.",
        ),
    ),
    (
        EslintRule::NoEmptyPattern,
        RuleText::new(
            "An empty destructuring pattern",
            "name what you want to extract or delete the pattern",
            "{} or [] in a pattern extracts nothing; a default value was almost always intended.",
            "const { a: {} } = object; becomes const { a = {} } = object;.",
        ),
    ),
    (
        EslintRule::NoSparseArrays,
        RuleText::new(
            "An array with holes",
            "write every element, with undefined if needed",
            "Two commas in a row leave a hole that looks like a typo and that array methods skip.",
            "[1, , 3] becomes [1, undefined, 3].",
        ),
    ),
    (
        EslintRule::RequireYield,
        RuleText::new(
            "A generator without yield",
            "add the yield or turn it into a regular function",
            "A function* without yield produces no values: either the yield is missing or it should not be a generator.",
            "function* ids() { return 1; } becomes function ids() { return 1; }.",
        ),
    ),
    (
        EslintRule::NoUselessCatch,
        RuleText::new(
            "A catch that only rethrows the error",
            "delete the try/catch, or the catch if there is a finally",
            "Catching an error just to throw it again adds noise without changing anything.",
            "try { load(); } catch (e) { throw e; } becomes load();.",
        ),
    ),
    (
        EslintRule::NoCompareNegZero,
        RuleText::new(
            "A comparison against -0",
            "use Object.is(value, -0)",
            "Comparisons do not tell -0 from 0: x === -0 is also true for 0.",
            "x === -0 becomes Object.is(x, -0).",
        ),
    ),
    (
        EslintRule::NoAsyncPromiseExecutor,
        RuleText::new(
            "An async function as the executor of new Promise",
            "drop the async and await outside, or return the promise directly",
            "If an async executor throws, the promise is never rejected: the error is lost.",
            "new Promise(async (resolve) => ...) becomes an async function that returns the value.",
        ),
    ),
    (
        EslintRule::UseIsnan,
        RuleText::new(
            "A comparison against NaN",
            "use Number.isNaN(value)",
            "NaN equals nothing, not even itself: the comparison always gives the same result.",
            "value === NaN becomes Number.isNaN(value).",
        ),
    ),
    (
        EslintRule::NoUnusedLabels,
        RuleText::new(
            "A label nobody uses",
            "delete it",
            "A label with no break or continue naming it is noise, or a mistyped object literal.",
            "outer: for (...) { ... } becomes for (...) { ... }.",
        ),
    ),
    (
        EslintRule::NoSelfAssign,
        RuleText::new(
            "An assignment of something to itself",
            "delete it or assign what was intended",
            "x = x changes nothing: it is almost always a typo on one of the sides.",
            "this.name = this.name; becomes this.name = name;.",
        ),
    ),
    (
        EslintRule::NoSetterReturn,
        RuleText::new(
            "A setter that returns a value",
            "drop the value from the return",
            "Whatever a setter returns is always discarded: a return with a value misleads the reader.",
            "set name(value) { return this._name = value; } becomes set name(value) { this._name = value; }.",
        ),
    ),
    (
        EslintRule::NoUnsafeOptionalChaining,
        RuleText::new(
            "A ?. where undefined throws",
            "make sure of the value first or keep using ?. along the chain",
            "If the optional chain short-circuits, the result is undefined and calling, destructuring or iterating it throws a TypeError.",
            "(object?.method)() becomes object?.method?.();.",
        ),
    ),
    (
        EslintRule::NoNonNullAssertedOptionalChain,
        RuleText::new(
            "A non-null ! on a ?.",
            "drop the ! or the ?.",
            "The ?. says there may be no value and the ! claims the opposite: one of them is wrong.",
            "user?.name! becomes user?.name or user.name.",
        ),
    ),
    (
        EslintRule::PreserveCaughtError,
        RuleText::new(
            "A new error thrown from a catch without the original",
            "pass the caught error as cause",
            "Without cause the original stack and message are lost: debugging turns into guessing.",
            "throw new Error('load failed'); becomes throw new Error('load failed', { cause: error });.",
        ),
    ),
    (
        EslintRule::NoExtraBooleanCast,
        RuleText::new(
            "A !! or Boolean() where the value is already used as a boolean",
            "drop the conversion",
            "An if, a ternary or a ! already converts to boolean: the extra conversion is noise.",
            "if (!!list.length) becomes if (list.length).",
        ),
    ),
    (
        EslintRule::NoNamespace,
        RuleText::new(
            "A TypeScript namespace instead of modules",
            "use ES import and export",
            "Namespaces predate ES modules: modern tools do not analyze them well and they are not needed.",
            "namespace Utils { export function f() {} } becomes export function f() {} in its own file.",
        ),
    ),
    (
        EslintRule::NoArrayConstructor,
        RuleText::new(
            "Array(...) or new Array(...) instead of a literal",
            "use the [ ] literal",
            "With a single number it creates an array of that length and with several, one with those elements: the literal has no such trap.",
            "new Array(1, 2) becomes [1, 2].",
        ),
    ),
    (
        EslintRule::NoUnreachable,
        RuleText::new(
            "Code that never runs",
            "delete it or check the return, throw, break or continue that cuts it off",
            "What follows a return or a throw never runs: either it is left over or the exit is in the wrong place.",
            "return total; log(total); becomes log(total); return total;.",
        ),
    ),
    (
        EslintRule::NoFallthrough,
        RuleText::new(
            "A case that falls through to the next one",
            "add break, or a // falls through comment if it is intended",
            "Falling through silently is one of the most common switch bugs: the next case's code runs too.",
            "case 1: load(); case 2: ... becomes case 1: load(); break; case 2: ...",
        ),
    ),
    (
        EslintRule::NoUselessAssignment,
        RuleText::new(
            "An assigned value that is never read",
            "delete the assignment or use the value",
            "The value is overwritten or discarded before anyone reads it: either the assignment is left over or a use is missing.",
            "let total = 0; total = compute(); becomes let total = compute();.",
        ),
    ),
    (
        EslintRule::GetterReturn,
        RuleText::new(
            "A getter that can finish without returning anything",
            "return a value on every path",
            "Whoever reads the property gets undefined on that path, almost always by accident.",
            "get total() { if (list) return sum(list); } becomes get total() { return list ? sum(list) : 0; }.",
        ),
    ),
    (
        EslintRule::NoLossOfPrecision,
        RuleText::new(
            "A number JavaScript cannot store as written",
            "write it with the precision a double holds, or use BigInt",
            "The value is rounded at compile time: the number that runs is not the one that is written.",
            "9007199254740993 becomes 9007199254740993n or 9007199254740992.",
        ),
    ),
    (
        EslintRule::NoControlRegex,
        RuleText::new(
            "A control character in a regular expression",
            "write the escape on purpose or remove it",
            "Characters U+0000 to U+001F almost never appear in searched text: it is usually an escaping mistake.",
            "/\\x1f/ becomes /\\s/ or whatever was meant.",
        ),
    ),
];
