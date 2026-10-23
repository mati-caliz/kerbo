mod checks;

use crate::i18n::catalog::RuleText;
use crate::rules::EslintRule;
use checks::CHECK_TEXTS;

const DEAD_BRANCH_WHY: &str =
    "A branch that can never run is a bug waiting: either the condition is wrong or the code is left over.";

const UNKNOWN: RuleText = RuleText::new(
    "An ESLint rule",
    "follow the rule",
    "ESLint reports it in its recommended set.",
    "See the ESLint documentation.",
);
const TEXTS: [(EslintRule, RuleText); 29] = [
    (
        EslintRule::NoVar,
        RuleText::new(
            "var instead of let or const",
            "declare it with let, or with const if it is never reassigned",
            "var ignores blocks and is hoisted to the whole function: the variable exists before its line and outside its block.",
            "for (var i = 0; ...) becomes for (let i = 0; ...).",
        ),
    ),
    (
        EslintRule::PreferConst,
        RuleText::new(
            "A let that is never reassigned",
            "declare it with const",
            "const tells the reader the value does not change and lets the compiler stop you if someone tries.",
            "let total = sum(items); return total; becomes const total = ...",
        ),
    ),
    (
        EslintRule::NoUnusedVars,
        RuleText::new(
            "A variable, function, import or parameter that is never used",
            "delete it, or use it if it was forgotten",
            "Something declared and never read is noise at best and, at worst, a sign that the code that used it got lost.",
            "import { format, parse } from 'date-fns' with only parse in use becomes import { parse } from 'date-fns'.",
        ),
    ),
    (
        EslintRule::NoUnusedExpressions,
        RuleText::new(
            "An expression whose result is discarded",
            "use the result, or turn it into the statement it meant to be",
            "An expression on its own does nothing: it is usually a missing call, return or assignment.",
            "isValid && save(); becomes if (isValid) save();",
        ),
    ),
    (
        EslintRule::NoRequireImports,
        RuleText::new(
            "require() instead of import",
            "use an ES import",
            "require is resolved at run time and the compiler cannot type or check what it brings in.",
            "const fs = require('fs') becomes import fs from 'fs'.",
        ),
    ),
    (
        EslintRule::NoExplicitAny,
        RuleText::new(
            "A value typed as any",
            "give it its real type; with external data, unknown plus validation",
            "any turns the compiler off for that value and for everything that touches it.",
            "function parse(input: any) becomes function parse(input: unknown) with a check before using it.",
        ),
    ),
    (
        EslintRule::NoNonNullAssertion,
        RuleText::new(
            "A non-null assertion with !",
            "check the value, or type it so it cannot be null",
            "The ! tells the compiler to trust you: if the value is null after all, it breaks at run time.",
            "user!.name becomes if (!user) throw ...; user.name.",
        ),
    ),
    (
        EslintRule::PreferRestParams,
        RuleText::new(
            "arguments instead of rest parameters",
            "declare a ...rest parameter",
            "arguments is not an array, does not work in arrow functions and hides what the function receives.",
            "function log() { use(arguments) } becomes function log(...messages) { use(messages) }.",
        ),
    ),
    (
        EslintRule::NoPrototypeBuiltins,
        RuleText::new(
            "A call to hasOwnProperty, isPrototypeOf or propertyIsEnumerable on the object itself",
            "use Object.hasOwn(object, key) or Object.prototype.hasOwnProperty.call",
            "The object can shadow the method or not have it at all (Object.create(null)): the call breaks or lies.",
            "data.hasOwnProperty(key) becomes Object.hasOwn(data, key).",
        ),
    ),
    (
        EslintRule::NoEmpty,
        RuleText::new(
            "An empty block",
            "fill it in, or leave a comment saying why it stays empty",
            "An empty block, above all an empty catch, swallows what happened without leaving a trace.",
            "catch (error) {} becomes catch (error) { logger.warn(error) } or a comment explaining it.",
        ),
    ),
    (
        EslintRule::NoUselessEscape,
        RuleText::new(
            "A backslash that escapes nothing",
            "delete the backslash",
            "An unnecessary escape adds noise and suggests the character means something it does not.",
            "'\\-' becomes '-' and /[\\/]/ becomes /[/]/.",
        ),
    ),
    (
        EslintRule::NoCondAssign,
        RuleText::new(
            "An assignment inside a condition",
            "compare with === or move the assignment out of the condition",
            "if (a = b) almost always meant if (a === b): it assigns and the condition depends on the value.",
            "if (user = find(id)) becomes const user = find(id); if (user).",
        ),
    ),
    (
        EslintRule::NoThisAlias,
        RuleText::new(
            "this saved in a variable",
            "use an arrow function, which keeps this",
            "const self = this is the workaround from before arrow functions and makes it unclear which object each line touches.",
            "const self = this; list.forEach(function () { self.add() }) becomes list.forEach(() => this.add()).",
        ),
    ),
    (
        EslintRule::BanTsComment,
        RuleText::new(
            "A @ts-ignore, @ts-nocheck or @ts-expect-error without a reason",
            "fix the type error; if it has to stay, @ts-expect-error with a description",
            "The comment turns the compiler off on that line or the whole file, and @ts-ignore stays silent even once the error is gone.",
            "// @ts-ignore becomes the real fix, or // @ts-expect-error: the library types are wrong (#123).",
        ),
    ),
    (
        EslintRule::NoEmptyObjectType,
        RuleText::new(
            "The {} type or an empty interface",
            "use object, unknown or Record<string, never>, or extend the type directly",
            "{} does not mean empty object: it accepts any value except null and undefined.",
            "function save(data: {}) becomes function save(data: object).",
        ),
    ),
    (
        EslintRule::NoCaseDeclarations,
        RuleText::new(
            "let, const, function or class directly inside a case",
            "wrap the case body in braces",
            "The declaration lives in the whole switch: other cases see it uninitialized and fail at run time.",
            "case 1: const total = ...; becomes case 1: { const total = ...; break; }",
        ),
    ),
    (
        EslintRule::NoDebugger,
        RuleText::new(
            "A debugger statement",
            "delete it",
            "A forgotten debugger stops the program for whoever has the developer tools open.",
            "debugger; is deleted before the commit.",
        ),
    ),
    (
        EslintRule::NoDuplicateCase,
        RuleText::new(
            "A case repeated in the same switch",
            "delete the repeated case or fix its value",
            DEAD_BRANCH_WHY,
            "case 'a': ... case 'a': becomes case 'a': ... case 'b':",
        ),
    ),
    (
        EslintRule::NoDupeElseIf,
        RuleText::new(
            "An else if whose condition was already covered by a previous one",
            "fix the condition or delete the branch",
            DEAD_BRANCH_WHY,
            "if (a) ... else if (a) ... becomes else if (b).",
        ),
    ),
    (
        EslintRule::NoDupeKeys,
        RuleText::new(
            "A key repeated in the same object",
            "delete one of the two or rename it",
            "The last one silently wins: the first value is lost and the reader cannot tell which one counts.",
            "{ id: 1, name: 'a', id: 2 } becomes { id: 2, name: 'a' }.",
        ),
    ),
    (
        EslintRule::NoRedeclare,
        RuleText::new(
            "A variable declared twice in the same scope",
            "drop the second declaration or use another name",
            "The second one silently overrides the first: readers can't tell which one holds or from where.",
            "var total = 0; ... var total = sum(); becomes total = sum();.",
        ),
    ),
    (
        EslintRule::NoFuncAssign,
        RuleText::new(
            "Something is assigned to the name of a declared function",
            "keep the value in another variable",
            "After the assignment, callers of the function call something else: it is almost always a bug.",
            "function load() {} load = other; becomes const loadOther = other;.",
        ),
    ),
    (
        EslintRule::NoWrapperObjectTypes,
        RuleText::new(
            "String, Number or Object as a type instead of the primitive",
            "use the lowercase primitive type",
            "String as a type also accepts the objects that wrap the primitive (new String('a')), which compare by reference and are almost never what you want.",
            "let name: String becomes let name: string.",
        ),
    ),
    (
        EslintRule::NoUnsafeFunctionType,
        RuleText::new(
            "Function as a type",
            "write the signature: its parameters and what it returns",
            "Function accepts anything callable and calling it returns any: all type checking is lost.",
            "callback: Function becomes callback: (error: Error | null) => void.",
        ),
    ),
    (
        EslintRule::TripleSlashReference,
        RuleText::new(
            "A /// <reference> directive instead of an import",
            "import the module or set types in the tsconfig",
            "The directive hides the dependency from the module system: tools cannot see it and the load order stays implicit.",
            "/// <reference path=\"./util.ts\" /> becomes import { util } from './util';.",
        ),
    ),
    (
        EslintRule::NoUnsafeFinally,
        RuleText::new(
            "A return, throw, break or continue inside a finally",
            "move it out of the finally",
            "Leaving a finally overrides what the try was doing: the returned value or the exception is lost without a trace.",
            "finally { return value; } becomes finally { close(); } with the return after the try.",
        ),
    ),
    (
        EslintRule::PreferSpread,
        RuleText::new(
            "f.apply(this, args) instead of the spread operator",
            "call the function with ...args",
            "Spread says the same thing without repeating this and without making the reader remember what apply does.",
            "Math.max.apply(Math, numbers) becomes Math.max(...numbers).",
        ),
    ),
    (
        EslintRule::NoConstantCondition,
        RuleText::new(
            "A condition that always evaluates the same",
            "use a condition that depends on something, or delete the branch that never runs",
            DEAD_BRANCH_WHY,
            "if (1) { ... } becomes the bare block; while (x = 0) becomes while (x === 0).",
        ),
    ),
    (
        EslintRule::NoConstantBinaryExpression,
        RuleText::new(
            "A comparison or logical operator that always gives the same result",
            "check the precedence or the operand that never changes",
            "It is almost always a precedence mistake or a literal nobody meant to compare: a + b ?? c never uses c.",
            "list === [] becomes list.length === 0; a + b ?? 0 becomes a + (b ?? 0).",
        ),
    ),
];

pub fn eslint(rule: EslintRule) -> RuleText {
    TEXTS
        .iter()
        .chain(CHECK_TEXTS.iter())
        .find(|(candidate, _)| *candidate == rule)
        .map_or(UNKNOWN, |(_, text)| *text)
}
