use std::slice::Iter;

use super::InvalidDocument;
use crate::formatter::document::element::{Element, TagKind};

// Lo que falta imprimir: una pila de tramos del documento, el de arriba primero.
pub trait ElementQueue<'document> {
    fn pop(&mut self) -> Option<&'document Element>;

    fn extend_back(&mut self, elements: &'document [Element]);

    fn pop_slice(&mut self) -> Option<&'document [Element]>;

    // El contenido que queda de una etiqueta ya abierta, sin su cierre.
    fn take_content(&mut self, kind: TagKind) -> Result<Vec<&'document Element>, InvalidDocument> {
        let mut content = Vec::new();
        let mut depth = 1_usize;
        loop {
            let element = self.pop().ok_or(InvalidDocument)?;
            match element {
                Element::Interned(interned) => {
                    self.extend_back(interned);
                    continue;
                }
                Element::Start(start) if start.kind() == kind => depth = depth.saturating_add(1),
                Element::End(end) if *end == kind => depth = depth.saturating_sub(1),
                Element::Space
                | Element::Line(_)
                | Element::ExpandParent
                | Element::LineSuffixBoundary
                | Element::Token(_)
                | Element::Text { .. }
                | Element::BestFitting { .. }
                | Element::Start(_)
                | Element::End(_) => {}
            }
            if depth == 0 {
                return Ok(content);
            }
            content.push(element);
        }
    }

    fn skip_content(&mut self, kind: TagKind) -> Result<(), InvalidDocument> {
        self.take_content(kind).map(drop)
    }
}

#[derive(Debug, Clone, Default)]
pub struct PrintQueue<'document> {
    slices: Vec<Iter<'document, Element>>,
}

impl<'document> PrintQueue<'document> {
    pub fn new(elements: &'document [Element]) -> Self {
        Self {
            slices: vec![elements.iter()],
        }
    }
}

impl<'document> ElementQueue<'document> for PrintQueue<'document> {
    fn pop(&mut self) -> Option<&'document Element> {
        loop {
            if let Some(element) = self.slices.last_mut()?.next() {
                return Some(element);
            }
            self.slices.pop();
        }
    }

    fn extend_back(&mut self, elements: &'document [Element]) {
        if !elements.is_empty() {
            self.slices.push(elements.iter());
        }
    }

    fn pop_slice(&mut self) -> Option<&'document [Element]> {
        self.slices.pop().map(|slice| slice.as_slice())
    }
}

// La cola con que se mide: arranca vacía y, cuando se le termina lo propio, sigue leyendo lo que le
// falta imprimir a la cola de verdad, sin consumirla.
#[derive(Debug)]
pub struct FitsQueue<'document, 'print> {
    own: PrintQueue<'document>,
    rest: &'print [Iter<'document, Element>],
}

impl<'document, 'print> FitsQueue<'document, 'print> {
    pub fn new(print_queue: &'print PrintQueue<'document>) -> Self {
        Self {
            own: PrintQueue { slices: Vec::new() },
            rest: &print_queue.slices,
        }
    }

    fn next_of_the_rest(&mut self) -> Option<&'document [Element]> {
        let (last, before) = self.rest.split_last()?;
        self.rest = before;
        Some(last.as_slice())
    }
}

impl<'document> ElementQueue<'document> for FitsQueue<'document, '_> {
    fn pop(&mut self) -> Option<&'document Element> {
        loop {
            if let Some(element) = self.own.pop() {
                return Some(element);
            }
            let slice = self.next_of_the_rest()?;
            self.own.extend_back(slice);
        }
    }

    fn extend_back(&mut self, elements: &'document [Element]) {
        self.own.extend_back(elements);
    }

    fn pop_slice(&mut self) -> Option<&'document [Element]> {
        self.own.pop_slice().or_else(|| self.next_of_the_rest())
    }
}
