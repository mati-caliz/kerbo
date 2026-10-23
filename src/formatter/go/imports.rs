use std::cmp::Ordering;
use std::ops::RangeInclusive;

use super::ast::{CommentGroup, Decl, File, ImportSpec, Spec};
use super::position::{LineTable, Pos};
use super::token::Token;

const QUOTES: [char; 2] = ['"', '`'];

struct Assignment {
    spec: usize,
    left: bool,
    group: usize,
}

// Port de `ast.SortImports`, que gofmt aplica antes de imprimir: ordena cada tanda de imports en
// líneas seguidas y les reasigna las posiciones, comentarios incluidos. No saca duplicados, pero sí
// junta las líneas en blanco entre el último import y el `)`, como Go.
pub fn sort(file: &mut File, lines: &mut LineTable) {
    let File { decls, comments, .. } = file;
    for decl in decls.iter_mut() {
        let Decl::Gen(decl) = decl else {
            break;
        };
        if decl.tok != Token::Import {
            break;
        }
        if !decl.lparen.is_valid() {
            continue;
        }
        for (start, end) in runs(&decl.specs, lines) {
            if let Some(run) = decl.specs.get_mut(start..end) {
                sort_run(run, comments, lines);
            }
        }
        if let Some(last) = decl.specs.last() {
            let last_line = lines.line(last.pos());
            let mut rparen_line = lines.line(decl.rparen);
            while rparen_line > last_line.saturating_add(1) {
                rparen_line = rparen_line.saturating_sub(1);
                lines.merge_line(rparen_line);
            }
        }
    }
}

// Las tandas de imports en líneas seguidas: una línea en blanco corta la tanda.
fn runs(specs: &[Spec], lines: &LineTable) -> Vec<(usize, usize)> {
    let mut found = Vec::new();
    let mut start = 0;
    for index in 1..=specs.len() {
        let breaks_run = specs
            .get(index)
            .zip(specs.get(index.saturating_sub(1)))
            .is_none_or(|(current, previous)| {
                lines.line(current.pos()) > lines.line(previous.end()).saturating_add(1)
            });
        if breaks_run {
            found.push((start, index));
            start = index;
        }
    }
    found
}

fn sort_run(specs: &mut [Spec], comments: &mut [CommentGroup], lines: &LineTable) {
    if specs.len() <= 1 {
        return;
    }
    let slots: Vec<(Pos, Pos)> = specs.iter().map(|spec| (spec.pos(), spec.end())).collect();
    let Some(range) = comment_range(&slots, comments, lines) else {
        reorder(specs, &slots, comments, &[]);
        return;
    };
    let assignments = assign_comments(&slots, comments, &range, lines);
    reorder(specs, &slots, comments, &assignments);
    if let Some(groups) = comments.get_mut(range) {
        groups.sort_by_key(CommentGroup::pos);
    }
}

fn comment_range(
    slots: &[(Pos, Pos)],
    comments: &[CommentGroup],
    lines: &LineTable,
) -> Option<RangeInclusive<usize>> {
    let begin = lines.line_start(lines.line(slots.first()?.0));
    let last_end = slots.last()?.1;
    let end_line = lines.line(last_end);
    let end = if end_line == lines.line_count() {
        last_end
    } else {
        lines.line_start(end_line.saturating_add(1))
    };
    let mut first = None;
    let mut last = None;
    for (index, group) in comments.iter().enumerate() {
        if group.end() >= end {
            break;
        }
        if begin <= group.pos() {
            first = first.or(Some(index));
            last = Some(index);
        }
    }
    Some(first?..=last?)
}

fn assign_comments(
    slots: &[(Pos, Pos)],
    comments: &[CommentGroup],
    range: &RangeInclusive<usize>,
    lines: &LineTable,
) -> Vec<Assignment> {
    let mut assignments = Vec::new();
    let mut spec: usize = 0;
    for group in range.clone() {
        let Some(position) = comments.get(group).map(CommentGroup::pos) else {
            continue;
        };
        while slots
            .get(spec.saturating_add(1))
            .is_some_and(|slot| slot.0 <= position)
        {
            spec = spec.saturating_add(1);
        }
        let start = slots.get(spec).map_or(Pos::NONE, |slot| slot.0);
        let mut left = false;
        if spec == 0 && start > position {
            left = true;
        } else if spec.saturating_add(1) < slots.len()
            && lines.line(start).saturating_add(1) == lines.line(position)
        {
            spec = spec.saturating_add(1);
            left = true;
        }
        assignments.push(Assignment { spec, left, group });
    }
    assignments
}

fn reorder(
    specs: &mut [Spec],
    slots: &[(Pos, Pos)],
    comments: &mut [CommentGroup],
    assignments: &[Assignment],
) {
    let mut order: Vec<usize> = (0..specs.len()).collect();
    order.sort_by(|first, second| compare(specs.get(*first), specs.get(*second)));
    let original: Vec<Spec> = specs.to_vec();
    for (slot, source) in order.iter().enumerate() {
        let (Some(target), Some(moved), Some((start, end))) =
            (specs.get_mut(slot), original.get(*source), slots.get(slot))
        else {
            continue;
        };
        *target = moved.clone();
        if let Spec::Import(import) = target {
            place(import, *start, *end);
        }
        for assignment in assignments.iter().filter(|assignment| assignment.spec == *source) {
            let slash = if assignment.left { start.back(1) } else { *end };
            move_group(comments, assignment.group, slash);
        }
    }
}

fn move_group(comments: &mut [CommentGroup], group: usize, slash: Pos) {
    if let Some(found) = comments.get_mut(group) {
        for comment in &mut found.list {
            comment.slash = slash;
        }
    }
}

fn place(import: &mut ImportSpec, start: Pos, end: Pos) {
    if let Some(name) = &mut import.name {
        name.pos = start;
    }
    let length = import
        .path
        .end
        .offset()
        .zip(import.path.pos.offset())
        .map_or(0, |(path_end, path_start)| path_end.saturating_sub(path_start));
    import.path.pos = start;
    import.path.end = start.shifted(length);
    import.end_pos = end;
}

fn compare(first: Option<&Spec>, second: Option<&Spec>) -> Ordering {
    let key = |spec: Option<&Spec>| match spec {
        Some(Spec::Import(import)) => (
            import.path.value.trim_matches(QUOTES).to_owned(),
            import
                .name
                .as_ref()
                .map(|name| name.name.clone())
                .unwrap_or_default(),
        ),
        Some(Spec::Value(_) | Spec::Type(_)) | None => (String::new(), String::new()),
    };
    key(first).cmp(&key(second))
}
