use std::cell::Cell;

use super::queue::{ElementQueue, FitsQueue};
use super::stack::{Args, CallStack, FitsStack, MeasureMode};
use super::{
    CLOSING_PARENTHESIS, Frames, InvalidDocument, LINE_WIDTH, Printer, SPACE, SPACE_WIDTH, indent_width,
};
use crate::formatter::document::element::{
    BestFittingMode, Condition, Element, GroupId, GroupMode, LineMode, PrintMode, Start, TagKind, char_width,
    clamped,
};

const NEWLINE: char = '\n';

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Fits {
    Yes,
    No,
    Maybe,
}

// Mide si lo que queda en la cola entra en la línea actual, sin imprimir nada: avanza sobre una
// vista de la cola y de la pila hasta el primer salto de línea o hasta pasarse del ancho.
#[derive(Debug)]
pub struct Measurer<'document, 'print> {
    queue: FitsQueue<'document, 'print>,
    stack: FitsStack<'print>,
    printer: &'print mut Printer<'document>,
    pending_indent: u16,
    line_width: u32,
    has_line_suffix: bool,
}

impl<'document, 'print> Measurer<'document, 'print> {
    pub fn new(printer: &'print mut Printer<'document>, frames: &'print Frames<'document>) -> Self {
        Self {
            queue: FitsQueue::new(&frames.queue),
            stack: FitsStack::new(&frames.stack),
            pending_indent: printer.pending_indent,
            line_width: printer.line_width,
            has_line_suffix: !printer.line_suffixes.is_empty(),
            printer,
        }
    }

    pub fn fits(&mut self) -> Result<bool, InvalidDocument> {
        while let Some(element) = self.queue.pop() {
            match self.fits_element(element)? {
                Fits::Yes => return Ok(true),
                Fits::No => return Ok(false),
                Fits::Maybe => {}
            }
        }
        Ok(true)
    }

    fn fits_element(&mut self, element: &'document Element) -> Result<Fits, InvalidDocument> {
        let args = self.stack.top();
        match element {
            Element::Space => Ok(self.fits_text(SPACE, Some(SPACE_WIDTH), args)),
            Element::Line(mode) => Ok(self.fits_line(*mode, args)),
            Element::Token(text) => Ok(self.fits_text(text, Some(clamped(text.len())), args)),
            Element::Text { text, width } => Ok(self.fits_text(text, *width, args)),
            Element::LineSuffixBoundary => Ok(Self::unless(self.has_line_suffix)),
            Element::ExpandParent => Ok(Fits::Maybe),
            Element::BestFitting { variants, mode } => self.fits_best_fitting(variants, *mode, args),
            Element::Interned(content) => {
                self.queue.extend_back(content);
                Ok(Fits::Maybe)
            }
            Element::Start(start) => self.fits_start(start, args),
            Element::End(kind) => self.fits_end(*kind, args),
        }
    }

    const fn unless(fails: bool) -> Fits {
        if fails { Fits::No } else { Fits::Maybe }
    }

    fn fits_line(&mut self, mode: LineMode, args: Args) -> Fits {
        match (args.mode, mode) {
            (PrintMode::Flat, LineMode::SoftOrSpace) => self.fits_text(SPACE, Some(SPACE_WIDTH), args),
            (PrintMode::Flat, LineMode::Soft) => Fits::Maybe,
            (PrintMode::Flat, LineMode::Hard | LineMode::Empty) => Fits::Yes,
            (PrintMode::Expanded, _) => match args.measure {
                MeasureMode::FirstLine => Fits::Yes,
                MeasureMode::AllLines | MeasureMode::AllLinesAllowTextOverflow => {
                    self.line_width = 0;
                    self.pending_indent = args.indent;
                    Fits::Maybe
                }
            },
        }
    }

    fn fits_best_fitting(
        &mut self,
        variants: &'document [Box<[Element]>],
        mode: BestFittingMode,
        args: Args,
    ) -> Result<Fits, InvalidDocument> {
        let (variant, entry_args) = match args.mode {
            PrintMode::Flat => (variants.first(), args.with_measure(mode.into())),
            PrintMode::Expanded => (variants.last(), args),
        };
        let Some((Element::Start(Start::BestFittingEntry), content)) =
            variant.and_then(|variant| variant.split_first())
        else {
            return Err(InvalidDocument);
        };
        self.stack.push(TagKind::BestFittingEntry, entry_args);
        self.queue.extend_back(content);
        Ok(Fits::Maybe)
    }

    fn fits_start(&mut self, start: &Start, args: Args) -> Result<Fits, InvalidDocument> {
        match start {
            Start::Indent => self.stack.push(TagKind::Indent, args.indented()),
            Start::Group { id, mode } => return Ok(self.fits_group(TagKind::Group, (*id, mode.get()), args)),
            Start::BestFitParenthesize { id } => {
                self.printer.remember(*id, args.mode);
                self.stack.push(TagKind::BestFitParenthesize, args);
            }
            Start::ConditionalGroup { condition, mode } => {
                if self.printer.condition_holds(*condition, args)? {
                    return Ok(self.fits_group(TagKind::ConditionalGroup, (None, mode.get()), args));
                }
                self.stack.push(TagKind::ConditionalGroup, args);
            }
            Start::ConditionalContent(condition) => {
                if self.printer.condition_holds(*condition, args)? {
                    self.stack.push(TagKind::ConditionalContent, args);
                } else {
                    self.queue.skip_content(TagKind::ConditionalContent)?;
                }
            }
            Start::IndentIfGroupBreaks(group) => {
                let indented = match self.printer.group_mode(*group)? {
                    PrintMode::Flat => args,
                    PrintMode::Expanded => args.indented(),
                };
                self.stack.push(TagKind::IndentIfGroupBreaks, indented);
            }
            Start::LineSuffix { reserved_width } => return self.fits_line_suffix(*reserved_width),
            Start::FitsExpanded {
                condition,
                propagate_expand,
            } => return self.fits_expanded(*condition, propagate_expand, args),
            Start::BestFittingEntry => self.stack.push(TagKind::BestFittingEntry, args),
        }
        Ok(Fits::Maybe)
    }

    fn fits_end(&mut self, kind: TagKind, args: Args) -> Result<Fits, InvalidDocument> {
        if kind == TagKind::LineSuffix {
            return Err(InvalidDocument);
        }
        let closes_added_parenthesis = kind == TagKind::BestFitParenthesize
            && args.mode == PrintMode::Expanded
            && self.stack.top_kind() == Some(TagKind::Indent);
        if closes_added_parenthesis {
            self.stack.pop(TagKind::Indent)?;
            let unindented = self.stack.pop(TagKind::BestFitParenthesize)?;
            self.line_width = 0;
            self.pending_indent = unindented.indent;
            return Ok(self.fits_text(CLOSING_PARENTHESIS, Some(SPACE_WIDTH), unindented));
        }
        self.stack.pop(kind)?;
        Ok(Fits::Maybe)
    }

    fn fits_group(&mut self, kind: TagKind, (id, mode): (Option<GroupId>, GroupMode), args: Args) -> Fits {
        let is_flat = mode == GroupMode::Flat;
        let print_mode = if is_flat { args.mode } else { PrintMode::Expanded };
        self.stack.push(kind, args.with_mode(print_mode));
        self.printer.remember(id, print_mode);
        Fits::Maybe
    }

    fn fits_line_suffix(&mut self, reserved_width: u32) -> Result<Fits, InvalidDocument> {
        if reserved_width > 0 {
            self.line_width = self.line_width.saturating_add(reserved_width);
            if self.line_width > LINE_WIDTH {
                return Ok(Fits::No);
            }
        }
        self.queue.skip_content(TagKind::LineSuffix)?;
        self.has_line_suffix = true;
        Ok(Fits::Maybe)
    }

    // Adentro de un grupo que se mide plano, el contenido de un `FitsExpanded` se mide partido y
    // sin límite de ancho: entra si lo que lo rodea entra.
    fn fits_expanded(
        &mut self,
        condition: Option<Condition>,
        propagate_expand: &Cell<bool>,
        args: Args,
    ) -> Result<Fits, InvalidDocument> {
        if args.mode == PrintMode::Expanded {
            self.stack.push(TagKind::FitsExpanded, args);
            return Ok(Fits::Maybe);
        }
        if self.printer.optional_condition_holds(condition, args)? {
            let expanded = args
                .with_measure(MeasureMode::AllLinesAllowTextOverflow)
                .with_mode(PrintMode::Expanded);
            self.stack.push(TagKind::FitsExpanded, expanded);
            return Ok(Fits::Maybe);
        }
        if propagate_expand.get() {
            return Ok(Fits::No);
        }
        self.stack.push(TagKind::FitsExpanded, args);
        Ok(Fits::Maybe)
    }

    const fn exceeds_width(&self, args: Args) -> bool {
        self.line_width > LINE_WIDTH && !matches!(args.measure, MeasureMode::AllLinesAllowTextOverflow)
    }

    fn fits_text(&mut self, text: &str, width: Option<u32>, args: Args) -> Fits {
        let indent = std::mem::take(&mut self.pending_indent);
        self.line_width = self.line_width.saturating_add(indent_width(indent));
        if let Some(width) = width {
            self.line_width = self.line_width.saturating_add(width);
        } else if let Some(decided) = self.fits_multiline_text(text, args) {
            return decided;
        }
        if self.exceeds_width(args) {
            Fits::No
        } else {
            Fits::Maybe
        }
    }

    fn fits_multiline_text(&mut self, text: &str, args: Args) -> Option<Fits> {
        for character in text.chars() {
            if character != NEWLINE {
                self.line_width = self.line_width.saturating_add(char_width(character));
                continue;
            }
            if args.measure == MeasureMode::FirstLine {
                return Some(self.fits_up_to_here(args));
            }
            self.line_width = 0;
        }
        None
    }

    const fn fits_up_to_here(&self, args: Args) -> Fits {
        if self.exceeds_width(args) {
            Fits::No
        } else {
            Fits::Yes
        }
    }
}
