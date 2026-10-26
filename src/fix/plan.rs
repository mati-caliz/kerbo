use super::{Fix, TextEdit, TextPoint};

// El `apply_fixes` de ruff: los arreglos por dónde empiezan (a igualdad, por su puesto en la lista de
// avisos de ruff y después en el orden en que llegan; el puesto 0 va antes que todo, como F811 en ruff), y en secuencia: uno se descarta si empieza antes del final del
// último aplicado (tocarse también cuenta), si ya se aplicó otro de su grupo de aislamiento, o si todas
// sus ediciones ya se aplicaron con otro.
pub fn compatible<'fix>(fixes: impl IntoIterator<Item = &'fix Fix>) -> Vec<&'fix Fix> {
    let mut ordered: Vec<&Fix> = fixes.into_iter().filter(|fix| !fix.edits.is_empty()).collect();
    ordered.sort_by_key(|fix| (fix.rank != 0, min_start(fix), fix.rank));
    let mut chosen: Vec<&Fix> = Vec::new();
    let mut applied: Vec<&TextEdit> = Vec::new();
    let mut last_end: Option<TextPoint> = None;
    let mut groups: Vec<usize> = Vec::new();
    for fix in ordered {
        let mut new_edits: Vec<&TextEdit> = fix.edits.iter().filter(|edit| !applied.contains(edit)).collect();
        new_edits.sort_by_key(|edit| edit.start);
        let Some(first) = new_edits.first() else {
            continue;
        };
        if let Some(group) = fix.isolation {
            if groups.contains(&group) {
                continue;
            }
            groups.push(group);
        }
        if last_end.is_some_and(|end| end >= first.start) {
            continue;
        }
        last_end = new_edits.iter().map(|edit| edit.end).max();
        applied.extend(new_edits);
        chosen.push(fix);
    }
    chosen
}

fn min_start(fix: &Fix) -> TextPoint {
    fix.edits
        .iter()
        .map(|edit| edit.start)
        .min()
        .unwrap_or(TextPoint { line: 0, column: 0 })
}

// Las ediciones de los arreglos elegidos, sin repetir las idénticas que comparten.
pub fn edits_of(fixes: &[&Fix]) -> Vec<TextEdit> {
    let mut edits: Vec<TextEdit> = Vec::new();
    for edit in fixes.iter().flat_map(|fix| fix.edits.iter()) {
        if !edits.contains(edit) {
            edits.push(edit.clone());
        }
    }
    edits
}
