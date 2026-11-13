use crate::i18n::catalog::RuleText;
use crate::lint::JavadocTagProblem;
use crate::rules::JavaRule;

pub fn tag_problem(problem: JavadocTagProblem, subject: &str) -> String {
    match problem {
        JavadocTagProblem::Missing => format!("The Javadoc lacks `{subject}`"),
        JavadocTagProblem::Unused => format!("`{subject}` matches nothing in the declaration"),
        JavadocTagProblem::Duplicate => format!("`{subject}` is repeated in the Javadoc"),
        JavadocTagProblem::Unknown => format!("`{subject}` is not a standard Javadoc block tag"),
        JavadocTagProblem::InvalidInheritDoc => {
            format!("`{subject}` inherits nothing on a constructor, a static method or a private one")
        }
    }
}

pub const RULE_COUNT: usize = 6;

const RULE_TEXTS: [(JavaRule, RuleText); RULE_COUNT] = [
    (
        JavaRule::JavadocVariable,
        RuleText::new(
            "Field or enum constant without Javadoc",
            "write a Javadoc saying what it holds",
            "sun_checks asks for Javadoc on every field, private ones too: what the value means, its unit and who changes it cannot be told from the name. serialVersionUID is left out. Only the strict preset asks for it.",
            "private int retries; gets /** How many times to retry before failing. */ above it.",
        ),
    ),
    (
        JavaRule::MissingJavadocMethod,
        RuleText::new(
            "Public method or constructor without Javadoc",
            "write a Javadoc with what it does, its parameters and what it returns",
            "The public members of a public type are the API: whoever uses it reads the Javadoc, not the body. Methods with @Override inherit the one they override. Only the strict preset asks for it.",
            "public Invoice issue(Order order) gets /** Issues the order's invoice. @param order ... @return ... */.",
        ),
    ),
    (
        JavaRule::JavadocMethod,
        RuleText::new(
            "Method Javadoc that does not match the signature",
            "add the missing tag or remove the extra one",
            "A Javadoc that exists has to document every parameter, every type parameter and the return value, with no tags for parameters that are gone: otherwise it lies. With {@inheritDoc} or @Override the missing ones are not asked for. Only the strict preset asks for it.",
            "A /** Adds. */ above int add(int a, int b) gets @param a, @param b and @return.",
        ),
    ),
    (
        JavaRule::JavadocType,
        RuleText::new(
            "Type Javadoc with an unknown tag or without the @param of a type parameter",
            "document every <T> and every record component, and use standard tags",
            "Like sun_checks' JavadocType: every type parameter (and every record component) gets its @param, and a block tag Checkstyle does not know, such as @apiNote, is reported. Only the strict preset asks for it.",
            "/** Cache. */ class Cache<K, V> gets @param <K> and @param <V>.",
        ),
    ),
    (
        JavaRule::JavadocPackage,
        RuleText::new(
            "Package without package-info.java",
            "add a package-info.java with the package's Javadoc",
            "The package Javadoc says what the package is for and what belongs in it; without package-info.java there is nowhere to write it. It is reported once per folder, on the first file in alphabetical order. Only the strict preset asks for it.",
            "src/main/java/com/payments gets package-info.java with /** Charges and refunds. */ package com.payments;.",
        ),
    ),
    (
        JavaRule::InvalidJavadocPosition,
        RuleText::new(
            "Javadoc in a place where it documents nothing",
            "move it above the declaration or turn it into a plain comment",
            "A /** */ inside a method, between an annotation and the declaration, or followed by another Javadoc shows up in no documentation: it is either misplaced or a plain comment. Only the strict preset asks for it.",
            "@Override /** Closes. */ public void close() becomes /** Closes. */ @Override public void close().",
        ),
    ),
];

pub fn rule_text(rule: JavaRule) -> Option<RuleText> {
    RULE_TEXTS
        .iter()
        .find(|(known, _)| *known == rule)
        .map(|(_, text)| *text)
}
