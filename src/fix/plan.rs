use super::{Fix, TextEdit};

pub fn compatible<'fix>(fixes: impl IntoIterator<Item = &'fix Fix>) -> Vec<&'fix Fix> {
    let mut chosen: Vec<&Fix> = Vec::new();
    for fix in fixes {
        let isolated = fix
            .isolation
            .is_some_and(|group| chosen.iter().any(|other| other.isolation == Some(group)));
        if !fix.edits.is_empty() && !isolated && chosen.iter().all(|other| !conflicts(fix, other)) {
            chosen.push(fix);
        }
    }
    chosen
}

pub fn edits_of(fixes: &[&Fix]) -> Vec<TextEdit> {
    fixes.iter().flat_map(|fix| fix.edits.iter().cloned()).collect()
}

fn conflicts(fix: &Fix, other: &Fix) -> bool {
    fix.edits
        .iter()
        .any(|edit| other.edits.iter().any(|known| overlaps(edit, known)))
}

fn overlaps(edit: &TextEdit, other: &TextEdit) -> bool {
    edit.start == other.start || (edit.start < other.end && other.start < edit.end)
}
