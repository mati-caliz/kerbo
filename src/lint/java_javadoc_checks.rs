use tree_sitter::{Node, Point};

use super::java::{
    IDENTIFIER, METHOD, PARAMETERS_FIELD, RECORD, STATIC, TYPE_FIELD, TYPE_IDENTIFIER, has_modifier,
    parameter_name, parameters, text,
};
use super::java_javadoc::{
    ANNOTATION_ELEMENT, COMPACT_CONSTRUCTOR, attached_javadoc, overrides, written_annotations,
};
use super::java_javadoc_scope::{Scope, declared_scope, has_javadoc_method_owner, surrounding_scope};
use super::java_javadoc_tags::{MethodTag, MethodTagKind, block_tags, method_tags};
use super::{JavadocTagProblem, LintFinding, LintKind, finding, finding_at};
use crate::rules::JavaRule;
use crate::syntax::{descendants, named_children};

const ERROR: &str = "ERROR";
const VARARGS: &str = "...";
const TYPE_PARAMETERS: &str = "type_parameters";
const TYPE_PARAMETER: &str = "type_parameter";
const VOID_TYPE: &str = "void_type";
const GENERATED: &str = "Generated";
const PARAM_TAG: &str = "@param";
const RETURN_TAG: &str = "@return";
const INHERIT_DOC_TAG: &str = "{@inheritDoc}";
const TYPE_PARAMETER_OPENER: char = '<';
const TYPE_PARAMETER_CLOSER: char = '>';

struct Named<'tree> {
    name: String,
    node: Node<'tree>,
}

fn type_parameter_nodes<'tree>(declaration: Node<'tree>, source: &[u8]) -> Vec<Named<'tree>> {
    named_children(declaration)
        .into_iter()
        .filter(|child| child.kind() == TYPE_PARAMETERS)
        .flat_map(named_children)
        .filter(|parameter| parameter.kind() == TYPE_PARAMETER)
        .filter_map(|parameter| {
            let identifier = named_children(parameter)
                .into_iter()
                .find(|part| part.kind() == TYPE_IDENTIFIER)?;
            Some(Named {
                name: text(identifier, source).to_owned(),
                node: parameter,
            })
        })
        .collect()
}

// tree-sitter-java no acepta una anotación antes de `...` (`Object @Nullable ... args`) y deja el
// parámetro como ERROR; Checkstyle sí lo lee, y su nombre es el último identificador del error.
fn annotated_varargs_name<'tree>(error: Node<'tree>, source: &[u8]) -> Option<Node<'tree>> {
    let is_varargs = error.kind() == ERROR && text(error, source).contains(VARARGS);
    is_varargs
        .then(|| descendants(error))?
        .into_iter()
        .rfind(|node| node.kind() == IDENTIFIER)
}

fn parameter_nodes<'tree>(declaration: Node<'tree>, source: &[u8]) -> Vec<Named<'tree>> {
    let declared = parameters(declaration).into_iter().filter_map(parameter_name);
    let recovered = declaration
        .child_by_field_name(PARAMETERS_FIELD)
        .map_or_else(Vec::new, named_children)
        .into_iter()
        .filter_map(|child| annotated_varargs_name(child, source));
    let mut names: Vec<Node<'tree>> = declared.chain(recovered).collect();
    names.sort_by_key(Node::start_byte);
    names
        .into_iter()
        .map(|name| Named {
            name: text(name, source).to_owned(),
            node: name,
        })
        .collect()
}

fn record_components<'tree>(declaration: Node<'tree>, source: &[u8]) -> Vec<Named<'tree>> {
    let record = if declaration.kind() == COMPACT_CONSTRUCTOR {
        std::iter::successors(declaration.parent(), Node::parent).find(|ancestor| ancestor.kind() == RECORD)
    } else {
        Some(declaration).filter(|node| node.kind() == RECORD)
    };
    record.map_or_else(Vec::new, |record| parameter_nodes(record, source))
}

fn bracketed(name: &str) -> String {
    format!("{TYPE_PARAMETER_OPENER}{name}{TYPE_PARAMETER_CLOSER}")
}

fn tagged(argument: &str) -> String {
    format!("{PARAM_TAG} {argument}")
}

fn tag_finding(rule: JavaRule, problem: JavadocTagProblem, row: usize, subject: &str) -> LintFinding {
    finding_at(
        LintKind::Javadoc(rule, problem),
        Point::new(row, 0),
        subject,
        None,
    )
}

fn method_tag_finding(problem: JavadocTagProblem, row: usize, subject: &str) -> LintFinding {
    tag_finding(JavaRule::JavadocMethod, problem, row, subject)
}

fn method_subject(tag: &MethodTag) -> String {
    match tag.kind {
        MethodTagKind::Param => tagged(&tag.argument),
        MethodTagKind::Return | MethodTagKind::Throws | MethodTagKind::See | MethodTagKind::InheritDoc => {
            RETURN_TAG.to_owned()
        }
    }
}

struct MethodJavadoc<'tree> {
    declaration: Node<'tree>,
    first_row: usize,
    tags: Vec<MethodTag>,
    findings: Vec<LintFinding>,
}

impl MethodJavadoc<'_> {
    const fn row(&self, tag: &MethodTag) -> usize {
        self.first_row.saturating_add(tag.row_offset)
    }

    fn take(&mut self, kind: MethodTagKind) -> Vec<MethodTag> {
        let (taken, kept) = std::mem::take(&mut self.tags)
            .into_iter()
            .partition(|tag| tag.kind == kind);
        self.tags = kept;
        taken
    }

    fn check_return(&mut self, report_expected: bool) {
        let returns = self.take(MethodTagKind::Return);
        for duplicate in returns.iter().skip(1) {
            let row = self.row(duplicate);
            self.findings
                .push(method_tag_finding(JavadocTagProblem::Duplicate, row, RETURN_TAG));
        }
        if returns.is_empty() && report_expected {
            let row = self.declaration.start_position().row;
            self.findings
                .push(method_tag_finding(JavadocTagProblem::Missing, row, RETURN_TAG));
        }
    }

    fn check_params(
        &mut self,
        mut declared: Vec<Named<'_>>,
        mut generics: Vec<Named<'_>>,
        report_expected: bool,
    ) {
        let names: Vec<String> = declared
            .iter()
            .map(|parameter| parameter.name.clone())
            .chain(generics.iter().map(|generic| bracketed(&generic.name)))
            .collect();
        for tag in self.take(MethodTagKind::Param) {
            if let Some(problem) = param_tag_problem(&tag.argument, &mut declared, &mut generics, &names) {
                self.findings.push(method_tag_finding(
                    problem,
                    self.row(&tag),
                    &tagged(&tag.argument),
                ));
            }
        }
        if report_expected {
            let missing = declared
                .iter()
                .map(|parameter| (parameter.node, parameter.name.clone()))
                .chain(
                    generics
                        .iter()
                        .map(|generic| (generic.node, bracketed(&generic.name))),
                );
            for (node, name) in missing {
                self.findings.push(finding(
                    LintKind::Javadoc(JavaRule::JavadocMethod, JavadocTagProblem::Missing),
                    node,
                    &tagged(&name),
                    None,
                ));
            }
        }
    }

    fn report_unused(&mut self) {
        let unused: Vec<LintFinding> = self
            .tags
            .iter()
            .filter(|tag| ![MethodTagKind::See, MethodTagKind::InheritDoc].contains(&tag.kind))
            .map(|tag| method_tag_finding(JavadocTagProblem::Unused, self.row(tag), &method_subject(tag)))
            .collect();
        self.findings.extend(unused);
    }
}

// Como `checkParamTags`: el @param consume el parámetro que nombra; si ya lo había consumido otro,
// está repetido.
fn param_tag_problem(
    argument: &str,
    declared: &mut Vec<Named<'_>>,
    generics: &mut Vec<Named<'_>>,
    names: &[String],
) -> Option<JavadocTagProblem> {
    let found = argument
        .strip_prefix(TYPE_PARAMETER_OPENER)
        .and_then(|rest| rest.strip_suffix(TYPE_PARAMETER_CLOSER))
        .map_or_else(
            || remove_named(declared, argument),
            |inner| remove_named(generics, inner),
        );
    if found {
        None
    } else if names.iter().any(|name| name == argument) {
        Some(JavadocTagProblem::Duplicate)
    } else {
        Some(JavadocTagProblem::Unused)
    }
}

fn remove_named(candidates: &mut Vec<Named<'_>>, wanted: &str) -> bool {
    candidates
        .iter()
        .position(|candidate| candidate.name == wanted)
        .map(|index| candidates.remove(index))
        .is_some()
}

fn inherits_documentation(declaration: Node<'_>) -> bool {
    declaration.kind() == METHOD
        && !has_modifier(declaration, STATIC)
        && declared_scope(declaration) != Scope::Private
}

fn returns_value(declaration: Node<'_>) -> bool {
    declaration.kind() == METHOD
        && declaration
            .child_by_field_name(TYPE_FIELD)
            .is_some_and(|returned| returned.kind() != VOID_TYPE)
}

// JavadocMethod: las etiquetas de un Javadoc que existe contra la firma; con {@inheritDoc} o
// @Override no se piden las que faltan, pero las que sobran sí se avisan.
pub fn method_tag_findings(declaration: Node<'_>, source: &[u8]) -> Vec<LintFinding> {
    let Some(comment) =
        attached_javadoc(declaration, source).filter(|_| has_javadoc_method_owner(declaration))
    else {
        return Vec::new();
    };
    let mut javadoc = MethodJavadoc {
        declaration,
        first_row: comment.start_position().row,
        tags: method_tags(text(comment, source)),
        findings: Vec::new(),
    };
    if let [only] = javadoc.tags.as_slice()
        && only.kind == MethodTagKind::InheritDoc
    {
        if !inherits_documentation(declaration) {
            let row = declaration.start_position().row;
            javadoc.findings.push(method_tag_finding(
                JavadocTagProblem::InvalidInheritDoc,
                row,
                INHERIT_DOC_TAG,
            ));
        }
        return javadoc.findings;
    }
    if declaration.kind() == ANNOTATION_ELEMENT {
        javadoc.check_return(true);
    } else {
        let inherits = javadoc
            .tags
            .iter()
            .any(|tag| tag.kind == MethodTagKind::InheritDoc);
        let report_expected = !inherits && !overrides(declaration, source);
        let declared = if declaration.kind() == COMPACT_CONSTRUCTOR {
            record_components(declaration, source)
        } else {
            parameter_nodes(declaration, source)
        };
        javadoc.check_params(
            declared,
            type_parameter_nodes(declaration, source),
            report_expected,
        );
        javadoc.take(MethodTagKind::Throws);
        if returns_value(declaration) {
            javadoc.check_return(report_expected);
        }
    }
    javadoc.report_unused();
    javadoc.findings
}

// JavadocType: etiquetas de bloque desconocidas, y un @param por parámetro de tipo o componente de
// record, sin repetir ni sobrar.
pub fn type_tag_findings(declaration: Node<'_>, source: &[u8]) -> Vec<LintFinding> {
    let skipped = surrounding_scope(declaration) == Scope::AnonymousInner
        || written_annotations(declaration, source).contains(&GENERATED);
    let Some(comment) = attached_javadoc(declaration, source).filter(|_| !skipped) else {
        return Vec::new();
    };
    let first_row = comment.start_position().row;
    let tags = block_tags(text(comment, source));
    let mut findings: Vec<LintFinding> = tags
        .iter()
        .filter(|tag| !tag.is_known())
        .map(|tag| {
            let row = first_row.saturating_add(tag.row_offset);
            let subject = format!("@{}", tag.name);
            tag_finding(JavaRule::JavadocType, JavadocTagProblem::Unknown, row, &subject)
        })
        .collect();
    let mut expected: Vec<String> = type_parameter_nodes(declaration, source)
        .iter()
        .map(|generic| bracketed(&generic.name))
        .chain(
            record_components(declaration, source)
                .into_iter()
                .map(|component| component.name),
        )
        .collect();
    for tag in tags.iter().filter(|tag| tag.is_param()) {
        if let Some(index) = expected.iter().position(|name| *name == tag.argument) {
            expected.remove(index);
        } else {
            let row = first_row.saturating_add(tag.row_offset);
            findings.push(tag_finding(
                JavaRule::JavadocType,
                JavadocTagProblem::Unused,
                row,
                &tagged(&tag.argument),
            ));
        }
    }
    let row = declaration.start_position().row;
    findings.extend(expected.iter().map(|name| {
        tag_finding(
            JavaRule::JavadocType,
            JavadocTagProblem::Missing,
            row,
            &tagged(name),
        )
    }));
    findings
}
