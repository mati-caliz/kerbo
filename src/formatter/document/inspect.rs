use super::element::{Element, GroupMode, LineMode, PrintMode, Start, TagKind};

fn element_will_break(element: &Element) -> bool {
    match element {
        Element::ExpandParent => true,
        Element::Start(Start::Group { mode, .. }) => mode.get() != GroupMode::Flat,
        Element::Line(mode) => matches!(mode, LineMode::Hard | LineMode::Empty),
        Element::Text { width, .. } => width.is_none(),
        Element::Interned(content) => will_break(content),
        Element::BestFitting { variants, .. } => {
            variants.first().is_some_and(|most_flat| will_break(most_flat))
        }
        Element::LineSuffixBoundary
        | Element::Space
        | Element::Token(_)
        | Element::Start(_)
        | Element::End(_) => false,
    }
}

// Si lo escrito tiene algo que obliga a partir la línea: un salto forzado, un texto de varias
// líneas o un grupo marcado para partirse. Lo que va adentro de un sufijo de línea no cuenta.
pub fn will_break(elements: &[Element]) -> bool {
    let mut ignore_depth = 0_usize;
    for element in elements {
        match element {
            Element::Start(Start::LineSuffix { .. } | Start::FitsExpanded { .. }) => {
                ignore_depth = ignore_depth.saturating_add(1);
            }
            Element::End(TagKind::LineSuffix | TagKind::FitsExpanded) => {
                ignore_depth = ignore_depth.saturating_sub(1);
            }
            Element::Space
            | Element::Line(_)
            | Element::ExpandParent
            | Element::LineSuffixBoundary
            | Element::Token(_)
            | Element::Text { .. }
            | Element::Interned(_)
            | Element::BestFitting { .. }
            | Element::Start(_)
            | Element::End(_) => {
                if ignore_depth == 0 && element_will_break(element) {
                    return true;
                }
            }
        }
    }
    false
}

// Cuántos `if_group_breaks` hay abiertos mientras se descarta su contenido.
#[derive(Debug, Default)]
struct Remover {
    cleaned: Vec<Element>,
    dropped_conditional_depth: usize,
}

impl Remover {
    fn drops(&mut self, element: &Element) -> bool {
        if self.dropped_conditional_depth > 0 {
            match element {
                Element::Start(Start::ConditionalContent(_)) => {
                    self.dropped_conditional_depth = self.dropped_conditional_depth.saturating_add(1);
                }
                Element::End(TagKind::ConditionalContent) => {
                    self.dropped_conditional_depth = self.dropped_conditional_depth.saturating_sub(1);
                }
                Element::Space
                | Element::Line(_)
                | Element::ExpandParent
                | Element::LineSuffixBoundary
                | Element::Token(_)
                | Element::Text { .. }
                | Element::Interned(_)
                | Element::BestFitting { .. }
                | Element::Start(_)
                | Element::End(_) => {}
            }
            return true;
        }
        match element {
            Element::Start(Start::ConditionalContent(condition)) => {
                if condition.mode == PrintMode::Expanded {
                    self.dropped_conditional_depth = 1;
                }
                true
            }
            Element::Line(LineMode::Soft)
            | Element::Start(Start::BestFittingEntry)
            | Element::End(TagKind::BestFittingEntry | TagKind::ConditionalContent) => true,
            Element::Space
            | Element::Line(_)
            | Element::ExpandParent
            | Element::LineSuffixBoundary
            | Element::Token(_)
            | Element::Text { .. }
            | Element::Interned(_)
            | Element::BestFitting { .. }
            | Element::Start(_)
            | Element::End(_) => false,
        }
    }

    fn push(&mut self, element: &Element) {
        if self.drops(element) {
            return;
        }
        match element {
            Element::Line(LineMode::SoftOrSpace) => self.cleaned.push(Element::Space),
            Element::Interned(content) => {
                let cleaned = remove_soft_line_breaks(content);
                self.cleaned.push(Element::Interned(cleaned.into()));
            }
            Element::BestFitting { variants, .. } => {
                for inner in variants.first().map(AsRef::as_ref).unwrap_or_default() {
                    self.push(inner);
                }
            }
            Element::Space
            | Element::Line(_)
            | Element::ExpandParent
            | Element::LineSuffixBoundary
            | Element::Token(_)
            | Element::Text { .. }
            | Element::Start(_)
            | Element::End(_) => self.cleaned.push(element.clone()),
        }
    }
}

// `RemoveSoftLinesBuffer` de ruff_formatter: el mismo contenido obligado a quedar en una línea. Los
// saltos blandos desaparecen o pasan a espacios, lo que sólo sale si un grupo se parte se descarta
// y de cada `best_fitting` queda la variante más plana.
pub fn remove_soft_line_breaks(elements: &[Element]) -> Vec<Element> {
    let mut remover = Remover::default();
    for element in elements {
        remover.push(element);
    }
    remover.cleaned
}
