use std::cell::Cell;
use std::collections::HashMap;

use super::element::{Element, GroupMode, LineMode, Start, TagKind};

#[derive(Debug)]
enum Enclosing<'document> {
    Group(&'document Cell<GroupMode>),
    FitsExpanded {
        propagate_expand: &'document Cell<bool>,
        expands_before: bool,
    },
    BestFitting,
    BestFitParenthesize {
        expanded: bool,
    },
}

type CheckedInterned = HashMap<*const Element, bool>;

fn expand_parent(enclosing: &[Enclosing<'_>]) {
    match enclosing.last() {
        Some(Enclosing::Group(mode)) => {
            if mode.get() == GroupMode::Flat {
                mode.set(GroupMode::Propagated);
            }
        }
        Some(Enclosing::FitsExpanded { propagate_expand, .. }) => propagate_expand.set(true),
        Some(Enclosing::BestFitting | Enclosing::BestFitParenthesize { .. }) | None => {}
    }
}

fn closes_expanded_group(enclosing: &mut Vec<Enclosing<'_>>) -> bool {
    match enclosing.pop() {
        Some(Enclosing::Group(mode)) => mode.get() != GroupMode::Flat,
        Some(
            Enclosing::FitsExpanded { .. } | Enclosing::BestFitting | Enclosing::BestFitParenthesize { .. },
        )
        | None => false,
    }
}

// Lo que un `BestFitParenthesize` o un `FitsExpanded` tienen adentro no parte a los grupos de
// afuera: al cerrarlos vuelve a valer lo que se venía acumulando antes de abrirlos.
fn restores(end: TagKind, enclosing: &mut Vec<Enclosing<'_>>) -> Option<bool> {
    if !matches!(end, TagKind::BestFitParenthesize | TagKind::FitsExpanded) {
        return None;
    }
    match enclosing.pop() {
        Some(
            Enclosing::BestFitParenthesize { expanded }
            | Enclosing::FitsExpanded {
                expands_before: expanded,
                ..
            },
        ) => Some(expanded),
        Some(Enclosing::Group(_) | Enclosing::BestFitting) | None => Some(false),
    }
}

fn opens<'document>(
    start: &'document Start,
    expands: bool,
    enclosing: &mut Vec<Enclosing<'document>>,
) -> Option<bool> {
    match start {
        Start::Group { mode, .. } | Start::ConditionalGroup { mode, .. } => {
            enclosing.push(Enclosing::Group(mode));
            None
        }
        Start::BestFitParenthesize { .. } => {
            enclosing.push(Enclosing::BestFitParenthesize { expanded: expands });
            Some(false)
        }
        Start::FitsExpanded { propagate_expand, .. } => {
            enclosing.push(Enclosing::FitsExpanded {
                propagate_expand,
                expands_before: expands,
            });
            None
        }
        Start::Indent
        | Start::ConditionalContent(_)
        | Start::IndentIfGroupBreaks(_)
        | Start::LineSuffix { .. }
        | Start::BestFittingEntry => None,
    }
}

fn interned_expands<'document>(
    content: &'document [Element],
    enclosing: &mut Vec<Enclosing<'document>>,
    checked: &mut CheckedInterned,
) -> bool {
    let key = content.as_ptr();
    if let Some(expands) = checked.get(&key) {
        return *expands;
    }
    let expands = propagate(content, enclosing, checked);
    checked.insert(key, expands);
    expands
}

fn propagate<'document>(
    elements: &'document [Element],
    enclosing: &mut Vec<Enclosing<'document>>,
    checked: &mut CheckedInterned,
) -> bool {
    let mut expands = false;
    for element in elements {
        let element_expands = match element {
            Element::Start(start) => {
                if let Some(reset) = opens(start, expands, enclosing) {
                    expands = reset;
                }
                false
            }
            Element::End(TagKind::Group | TagKind::ConditionalGroup) => closes_expanded_group(enclosing),
            Element::End(end) => {
                if let Some(restored) = restores(*end, enclosing) {
                    expands = restored;
                }
                false
            }
            Element::Interned(content) => interned_expands(content, enclosing, checked),
            Element::BestFitting { variants, .. } => {
                enclosing.push(Enclosing::BestFitting);
                for variant in variants {
                    propagate(variant, enclosing, checked);
                }
                enclosing.pop();
                false
            }
            Element::Text { width, .. } => width.is_none(),
            Element::ExpandParent | Element::Line(LineMode::Hard | LineMode::Empty) => true,
            Element::Space
            | Element::Token(_)
            | Element::LineSuffixBoundary
            | Element::Line(LineMode::Soft | LineMode::SoftOrSpace) => false,
        };
        if element_expands {
            expands = true;
            expand_parent(enclosing);
        }
    }
    expands
}

// Un salto de línea forzado, un texto de varias líneas o un `ExpandParent` parten todos los grupos
// que los encierran: se marca antes de imprimir, así el impresor no mide un grupo que ya se sabe
// que no entra en una línea.
pub fn propagate_expand(elements: &[Element]) {
    propagate(elements, &mut Vec::new(), &mut HashMap::new());
}
