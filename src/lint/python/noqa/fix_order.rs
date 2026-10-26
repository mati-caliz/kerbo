use crate::lint::python::Report;
use crate::rules::PythonRule;

const SYNTAX_TREE_STAGE: u8 = 0;
const DEFERRED_SCOPES_STAGE: u8 = 1;
const IMPORTS_STAGE: u8 = 2;
const DIRECTIVES_STAGE: u8 = 3;
// Ruff aplica los arreglos de F811 antes que todos los demás, sin importar dónde empiezan.
const FIRST_RANK: usize = 0;

// El puesto de cada arreglo en la lista de avisos que ruff le pasa a `apply_fixes`, que desempata dos
// arreglos que empiezan en el mismo lugar: primero los del árbol, después los de los alcances
// diferidos (F401, F841, RUF059), los de imports (I001) y al final los RUF100; los silenciados por
// un `noqa` se sacan con `swap_remove` desde el último, así que los últimos avisos pasan a ocupar
// su lugar.
pub fn rank_fixes_like_ruff(reports: &mut [Report], silenced: &[bool]) {
    let mut sequence: Vec<usize> = (0..reports.len()).collect();
    sequence.sort_by_key(|index| {
        reports
            .get(*index)
            .map_or(SYNTAX_TREE_STAGE, |report| generation_stage(report.rule))
    });
    let silenced_positions: Vec<usize> = sequence
        .iter()
        .enumerate()
        .filter(|(_, index)| silenced.get(**index).copied().unwrap_or(false))
        .map(|(position, _)| position)
        .collect();
    for position in silenced_positions.into_iter().rev() {
        sequence.swap_remove(position);
    }
    for (position, index) in sequence.into_iter().enumerate() {
        if let Some(report) = reports.get_mut(index) {
            let rank = if report.rule == PythonRule::F811 {
                FIRST_RANK
            } else {
                position.saturating_add(1)
            };
            if let Some(fix) = report.fix.as_mut() {
                fix.rank = rank;
            }
        }
    }
}

const fn generation_stage(rule: PythonRule) -> u8 {
    if matches!(rule, PythonRule::I001) {
        IMPORTS_STAGE
    } else if matches!(rule, PythonRule::RUF100) {
        DIRECTIVES_STAGE
    } else if matches!(rule, PythonRule::F401 | PythonRule::F841 | PythonRule::RUF059) {
        DEFERRED_SCOPES_STAGE
    } else {
        SYNTAX_TREE_STAGE
    }
}
