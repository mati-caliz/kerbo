mod best_fitting;
mod fits;
mod queue;
mod stack;

use std::collections::HashMap;
use std::slice::from_ref;

use super::element::{
    Condition, Element, GroupId, GroupMode, INDENT_WIDTH, LineMode, PrintMode, Start, TagKind, char_width,
    clamped,
};
use fits::Measurer;
use queue::{ElementQueue, PrintQueue};
use stack::{Args, CallStack, PrintStack};

const LINE_WIDTH: u32 = 88;
const NEWLINE: char = '\n';
const INDENT_CHARACTER: char = ' ';
const SPACE: &str = " ";
const SPACE_WIDTH: u32 = 1;
const OPENING_PARENTHESIS: &str = "(";
const CLOSING_PARENTHESIS: &str = ")";

// Una etiqueta que cierra sin haber abierto, o un grupo que se consulta antes de imprimirse: el
// documento está mal armado y no se imprime nada.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InvalidDocument;

// Elementos que el impresor encola por su cuenta: el paréntesis que agrega un
// `BestFitParenthesize` que no entra en la línea y el salto con que se vacían los sufijos.
#[derive(Debug)]
struct Sentinels {
    hard_break: Element,
    line_suffix_end: Element,
    open_parenthesis: [Element; 3],
    close_parenthesis: [Element; 2],
}

impl Sentinels {
    const fn new() -> Self {
        Self {
            hard_break: Element::Line(LineMode::Hard),
            line_suffix_end: Element::End(TagKind::LineSuffix),
            open_parenthesis: [
                Element::Token(OPENING_PARENTHESIS),
                Element::Start(Start::Indent),
                Element::Line(LineMode::Hard),
            ],
            close_parenthesis: [Element::Line(LineMode::Hard), Element::Token(CLOSING_PARENTHESIS)],
        }
    }
}

#[derive(Debug)]
struct Frames<'document> {
    queue: PrintQueue<'document>,
    stack: PrintStack,
}

#[derive(Debug, Clone, Copy)]
enum LineSuffix<'document> {
    Content(&'document Element),
    Args(Args),
}

fn indent_width(level: u16) -> u32 {
    u32::from(level).saturating_mul(INDENT_WIDTH)
}

// Port del impresor de ruff_formatter: recorre el documento una vez y, al abrir cada grupo, mide si
// entra en lo que queda de la línea; si no entra, sus saltos blandos se vuelven saltos de línea.
#[derive(Debug)]
struct Printer<'document> {
    sentinels: &'document Sentinels,
    buffer: String,
    pending_indent: u16,
    measured_group_fits: bool,
    line_start: usize,
    line_width: u32,
    group_modes: HashMap<GroupId, PrintMode>,
    line_suffixes: Vec<LineSuffix<'document>>,
}

pub fn print(document: &[Element]) -> Result<String, InvalidDocument> {
    let sentinels = Sentinels::new();
    let mut printer = Printer {
        sentinels: &sentinels,
        buffer: String::new(),
        pending_indent: 0,
        measured_group_fits: false,
        line_start: 0,
        line_width: 0,
        group_modes: HashMap::new(),
        line_suffixes: Vec::new(),
    };
    let mut frames = Frames {
        queue: PrintQueue::new(document),
        stack: PrintStack::new(),
    };
    loop {
        if let Some(element) = frames.queue.pop() {
            printer.print_element(element, &mut frames)?;
        } else if !printer.flush_line_suffixes(&mut frames, None) {
            break;
        }
    }
    Ok(printer.buffer)
}

impl<'document> Printer<'document> {
    fn print_element(
        &mut self,
        element: &'document Element,
        frames: &mut Frames<'document>,
    ) -> Result<(), InvalidDocument> {
        match element {
            Element::Space => self.print_text(SPACE, Some(SPACE_WIDTH)),
            Element::Token(text) => self.print_text(text, Some(clamped(text.len()))),
            Element::Text { text, width } => self.print_text(text, *width),
            Element::Line(mode) => self.print_line(*mode, element, frames),
            Element::ExpandParent => {}
            Element::LineSuffixBoundary => {
                let hard_break = &self.sentinels.hard_break;
                self.flush_line_suffixes(frames, Some(hard_break));
            }
            Element::BestFitting { variants, mode } => best_fitting::print(self, (variants, *mode), frames)?,
            Element::Interned(content) => frames.queue.extend_back(content),
            Element::Start(start) => self.print_start(start, frames)?,
            Element::End(kind) => self.print_end(*kind, frames)?,
        }
        Ok(())
    }

    fn print_line(&mut self, mode: LineMode, element: &'document Element, frames: &mut Frames<'document>) {
        let args = frames.stack.top();
        let is_soft = matches!(mode, LineMode::Soft | LineMode::SoftOrSpace);
        if args.mode == PrintMode::Flat && is_soft {
            if mode == LineMode::SoftOrSpace {
                self.print_text(SPACE, Some(SPACE_WIDTH));
            }
            return;
        }
        if self.flush_line_suffixes(frames, Some(element)) {
            return;
        }
        if self.buffer.len() > self.line_start {
            self.print_char(NEWLINE);
        }
        if mode == LineMode::Empty {
            self.print_char(NEWLINE);
        }
        self.pending_indent = args.indent;
    }

    // Los comentarios de fin de línea esperan acá hasta el próximo salto: se vuelven a encolar
    // delante del salto, que se imprime de nuevo cuando ya no queda ninguno.
    fn flush_line_suffixes(
        &mut self,
        frames: &mut Frames<'document>,
        line_break: Option<&'document Element>,
    ) -> bool {
        if self.line_suffixes.is_empty() {
            return false;
        }
        if let Some(line_break) = line_break {
            frames.queue.extend_back(from_ref(line_break));
        }
        let line_suffix_end = &self.sentinels.line_suffix_end;
        for suffix in std::mem::take(&mut self.line_suffixes).into_iter().rev() {
            match suffix {
                LineSuffix::Content(element) => frames.queue.extend_back(from_ref(element)),
                LineSuffix::Args(args) => {
                    frames.stack.push(TagKind::LineSuffix, args);
                    frames.queue.extend_back(from_ref(line_suffix_end));
                }
            }
        }
        true
    }

    fn print_start(
        &mut self,
        start: &'document Start,
        frames: &mut Frames<'document>,
    ) -> Result<(), InvalidDocument> {
        let args = frames.stack.top();
        match start {
            Start::Indent => frames.stack.push(TagKind::Indent, args.indented()),
            Start::Group { id, mode } => {
                let print_mode = self.group_print_mode(TagKind::Group, (*id, mode.get()), frames)?;
                self.remember(*id, print_mode);
                frames.stack.push(TagKind::Group, args.with_mode(print_mode));
            }
            Start::ConditionalGroup { condition, mode } => {
                self.print_conditional_group(*condition, mode.get(), frames)?;
            }
            Start::ConditionalContent(condition) => {
                if self.condition_holds(*condition, args)? {
                    frames.stack.push(TagKind::ConditionalContent, args);
                } else {
                    frames.queue.skip_content(TagKind::ConditionalContent)?;
                }
            }
            Start::IndentIfGroupBreaks(group) => {
                let indented = match self.group_mode(*group)? {
                    PrintMode::Flat => args,
                    PrintMode::Expanded => args.indented(),
                };
                frames.stack.push(TagKind::IndentIfGroupBreaks, indented);
            }
            Start::BestFittingEntry => frames.stack.push(TagKind::BestFittingEntry, args),
            Start::LineSuffix { reserved_width } => self.hold_line_suffix(*reserved_width, frames)?,
            Start::FitsExpanded { condition, .. } => {
                if self.optional_condition_holds(*condition, args)? {
                    self.measured_group_fits = false;
                }
                frames.stack.push(TagKind::FitsExpanded, args);
            }
            Start::BestFitParenthesize { id } => best_fitting::start_parenthesize(self, *id, frames)?,
        }
        Ok(())
    }

    fn print_end(&self, kind: TagKind, frames: &mut Frames<'document>) -> Result<(), InvalidDocument> {
        if kind == TagKind::BestFitParenthesize && frames.stack.top().mode == PrintMode::Expanded {
            frames.stack.pop(TagKind::Indent)?;
            frames.queue.extend_back(&self.sentinels.close_parenthesis);
        }
        frames.stack.pop(kind).map(drop)
    }

    fn print_conditional_group(
        &mut self,
        condition: Condition,
        mode: GroupMode,
        frames: &mut Frames<'document>,
    ) -> Result<(), InvalidDocument> {
        let args = frames.stack.top();
        if self.condition_holds(condition, args)? {
            let print_mode = self.group_print_mode(TagKind::ConditionalGroup, (None, mode), frames)?;
            frames
                .stack
                .push(TagKind::ConditionalGroup, args.with_mode(print_mode));
        } else {
            frames.stack.push(TagKind::ConditionalGroup, args);
        }
        Ok(())
    }

    fn hold_line_suffix(
        &mut self,
        reserved_width: u32,
        frames: &mut Frames<'document>,
    ) -> Result<(), InvalidDocument> {
        self.line_width = self.line_width.saturating_add(reserved_width);
        let content = frames.queue.take_content(TagKind::LineSuffix)?;
        self.line_suffixes
            .extend(content.into_iter().map(LineSuffix::Content));
        self.line_suffixes.push(LineSuffix::Args(frames.stack.top()));
        Ok(())
    }

    fn remember(&mut self, id: Option<GroupId>, mode: PrintMode) {
        if let Some(id) = id {
            self.group_modes.insert(id, mode);
        }
    }

    fn group_mode(&self, group: GroupId) -> Result<PrintMode, InvalidDocument> {
        self.group_modes.get(&group).copied().ok_or(InvalidDocument)
    }

    fn condition_holds(&self, condition: Condition, args: Args) -> Result<bool, InvalidDocument> {
        let mode = match condition.group {
            None => args.mode,
            Some(group) => self.group_mode(group)?,
        };
        Ok(mode == condition.mode)
    }

    fn optional_condition_holds(
        &self,
        condition: Option<Condition>,
        args: Args,
    ) -> Result<bool, InvalidDocument> {
        condition.map_or(Ok(true), |condition| self.condition_holds(condition, args))
    }

    fn group_print_mode(
        &mut self,
        kind: TagKind,
        (id, mode): (Option<GroupId>, GroupMode),
        frames: &mut Frames<'document>,
    ) -> Result<PrintMode, InvalidDocument> {
        match mode {
            GroupMode::Expand | GroupMode::Propagated => Ok(PrintMode::Expanded),
            GroupMode::Flat => self.flat_group_print_mode(kind, id, frames),
        }
    }

    // Un grupo adentro de otro que ya se midió entero y entra, entra: no se vuelve a medir.
    fn flat_group_print_mode(
        &mut self,
        kind: TagKind,
        id: Option<GroupId>,
        frames: &mut Frames<'document>,
    ) -> Result<PrintMode, InvalidDocument> {
        let args = frames.stack.top();
        if args.mode == PrintMode::Flat && self.measured_group_fits {
            return Ok(PrintMode::Flat);
        }
        self.measured_group_fits = true;
        self.remember(id, PrintMode::Flat);
        frames.stack.push(kind, args.with_mode(PrintMode::Flat));
        let fits = self.fits(frames)?;
        frames.stack.pop(kind)?;
        Ok(if fits {
            PrintMode::Flat
        } else {
            PrintMode::Expanded
        })
    }

    fn fits(&mut self, frames: &Frames<'document>) -> Result<bool, InvalidDocument> {
        Measurer::new(self, frames).fits()
    }

    fn print_entry(
        &mut self,
        frames: &mut Frames<'document>,
        args: Args,
        kind: TagKind,
    ) -> Result<(), InvalidDocument> {
        match frames.queue.pop() {
            Some(Element::Start(start)) if start.kind() == kind => frames.stack.push(kind, args),
            Some(_) | None => return Err(InvalidDocument),
        }
        let mut depth = 1_usize;
        while let Some(element) = frames.queue.pop() {
            match element {
                Element::Start(Start::BestFittingEntry) => depth = depth.saturating_add(1),
                Element::End(TagKind::BestFittingEntry) => depth = depth.saturating_sub(1),
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
            if depth == 0 {
                return frames.stack.pop(kind).map(drop);
            }
            self.print_element(element, frames)?;
        }
        Err(InvalidDocument)
    }

    fn print_text(&mut self, text: &str, width: Option<u32>) {
        let indent = std::mem::take(&mut self.pending_indent);
        let indent_columns = indent_width(indent);
        self.buffer.extend((0..indent_columns).map(|_| INDENT_CHARACTER));
        self.line_width = self.line_width.saturating_add(indent_columns);
        if let Some(width) = width {
            self.buffer.push_str(text);
            self.line_width = self.line_width.saturating_add(width);
        } else {
            text.chars().for_each(|character| self.print_char(character));
        }
    }

    fn print_char(&mut self, character: char) {
        self.buffer.push(character);
        if character == NEWLINE {
            self.line_width = 0;
            self.line_start = self.buffer.len();
            self.measured_group_fits = false;
        } else {
            self.line_width = self.line_width.saturating_add(char_width(character));
        }
    }
}
