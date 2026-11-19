use std::cell::Cell;
use std::rc::Rc;

use super::element::{
    BestFittingMode, Condition, Element, GroupId, GroupMode, LineMode, PrintMode, Start, TagKind, text_width,
};
use super::expand::propagate_expand;

// Arma el documento con los mismos nombres que los builders de ruff_formatter. Lo que abre un
// `start_*` lo cierra el `end_*` del mismo nombre.
#[derive(Debug, Default)]
pub struct Builder {
    elements: Vec<Element>,
    group_count: usize,
}

impl Builder {
    pub fn token(&mut self, text: &'static str) {
        self.elements.push(Element::Token(text));
    }

    pub fn text(&mut self, text: &str) {
        self.elements.push(Element::Text {
            text: text.into(),
            width: text_width(text),
        });
    }

    pub fn space(&mut self) {
        self.elements.push(Element::Space);
    }

    pub fn line(&mut self, mode: LineMode) {
        self.elements.push(Element::Line(mode));
    }

    pub fn expand_parent(&mut self) {
        self.elements.push(Element::ExpandParent);
    }

    pub fn line_suffix_boundary(&mut self) {
        self.elements.push(Element::LineSuffixBoundary);
    }

    pub fn push(&mut self, element: Element) {
        self.elements.push(element);
    }

    pub const fn group_id(&mut self) -> GroupId {
        let id = GroupId::new(self.group_count);
        self.group_count = self.group_count.saturating_add(1);
        id
    }

    pub fn start(&mut self, start: Start) {
        self.elements.push(Element::Start(start));
    }

    pub fn end(&mut self, kind: TagKind) {
        self.elements.push(Element::End(kind));
    }

    pub fn start_group(&mut self, id: Option<GroupId>) {
        self.start_group_expanding(id, false);
    }

    // `group(..).should_expand(expand)`.
    pub fn start_group_expanding(&mut self, id: Option<GroupId>, expand: bool) {
        let mode = if expand {
            GroupMode::Expand
        } else {
            GroupMode::Flat
        };
        self.start(Start::Group {
            id,
            mode: Cell::new(mode),
        });
    }

    pub fn end_group(&mut self) {
        self.end(TagKind::Group);
    }

    pub fn start_conditional_group(&mut self, condition: Condition) {
        self.start(Start::ConditionalGroup {
            condition,
            mode: Cell::new(GroupMode::Flat),
        });
    }

    pub fn start_indent(&mut self) {
        self.start(Start::Indent);
    }

    pub fn end_indent(&mut self) {
        self.end(TagKind::Indent);
    }

    pub fn start_indent_if_group_breaks(&mut self, group: GroupId) {
        self.start(Start::IndentIfGroupBreaks(group));
    }

    pub fn end_indent_if_group_breaks(&mut self) {
        self.end(TagKind::IndentIfGroupBreaks);
    }

    pub fn start_conditional_content(&mut self, mode: PrintMode, group: Option<GroupId>) {
        self.start(Start::ConditionalContent(Condition { mode, group }));
    }

    pub fn end_conditional_content(&mut self) {
        self.end(TagKind::ConditionalContent);
    }

    pub fn start_fits_expanded(&mut self, condition: Option<Condition>) {
        self.start(Start::FitsExpanded {
            condition,
            propagate_expand: Cell::new(false),
        });
    }

    pub fn start_line_suffix(&mut self, reserved_width: u32) {
        self.start(Start::LineSuffix { reserved_width });
    }

    // `if_group_breaks(&token(..))`: el texto sólo sale si el grupo que lo encierra se parte.
    pub fn token_if_group_breaks(&mut self, text: &'static str) {
        self.token_if(PrintMode::Expanded, None, text);
    }

    pub fn token_if(&mut self, mode: PrintMode, group: Option<GroupId>, text: &'static str) {
        self.start_conditional_content(mode, group);
        self.token(text);
        self.end_conditional_content();
    }

    pub const fn len(&self) -> usize {
        self.elements.len()
    }

    pub fn truncate(&mut self, length: usize) {
        self.elements.truncate(length);
    }

    // Lo escrito desde `length` sale del documento y vuelve como un tramo compartido: `memoized()`
    // de ruff, para escribir el mismo contenido en más de una variante sin formatearlo otra vez.
    pub fn take_since(&mut self, length: usize) -> Rc<[Element]> {
        let length = length.min(self.elements.len());
        self.elements.split_off(length).into()
    }

    pub fn interned(&mut self, content: &Rc<[Element]>) {
        if !content.is_empty() {
            self.elements.push(Element::Interned(Rc::clone(content)));
        }
    }

    // Cada variante llega sin sus etiquetas de entrada: se agregan acá, como `best_fitting!`.
    pub fn best_fitting(&mut self, variants: Vec<Rc<[Element]>>, mode: BestFittingMode) {
        let variants = variants
            .into_iter()
            .map(|variant| {
                let mut entry = Vec::with_capacity(variant.len().saturating_add(2));
                entry.push(Element::Start(Start::BestFittingEntry));
                entry.extend(variant.iter().cloned());
                entry.push(Element::End(TagKind::BestFittingEntry));
                entry.into_boxed_slice()
            })
            .collect();
        self.elements.push(Element::BestFitting { variants, mode });
    }

    pub fn finish(self) -> Vec<Element> {
        propagate_expand(&self.elements);
        self.elements
    }
}
