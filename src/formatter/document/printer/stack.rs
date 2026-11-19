use super::InvalidDocument;
use crate::formatter::document::element::{BestFittingMode, PrintMode, TagKind};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MeasureMode {
    FirstLine,
    AllLines,
    AllLinesAllowTextOverflow,
}

impl From<BestFittingMode> for MeasureMode {
    fn from(mode: BestFittingMode) -> Self {
        match mode {
            BestFittingMode::FirstLine => Self::FirstLine,
            BestFittingMode::AllLines => Self::AllLines,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Args {
    pub indent: u16,
    pub mode: PrintMode,
    pub measure: MeasureMode,
}

impl Args {
    const ROOT: Self = Self {
        indent: 0,
        mode: PrintMode::Expanded,
        measure: MeasureMode::FirstLine,
    };

    pub const fn indented(self) -> Self {
        Self {
            indent: self.indent.saturating_add(1),
            ..self
        }
    }

    pub const fn with_mode(self, mode: PrintMode) -> Self {
        Self { mode, ..self }
    }

    pub const fn with_measure(self, measure: MeasureMode) -> Self {
        Self { measure, ..self }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Frame {
    kind: Option<TagKind>,
    args: Args,
}

const ROOT_FRAME: Frame = Frame {
    kind: None,
    args: Args::ROOT,
};

fn popped(frame: Option<Frame>, kind: TagKind) -> Result<Args, InvalidDocument> {
    match frame {
        Some(Frame {
            kind: Some(actual),
            args,
        }) if actual == kind => Ok(args),
        Some(Frame { .. }) | None => Err(InvalidDocument),
    }
}

// Los argumentos con que se imprime cada etiqueta abierta; abajo de todo, los de la raíz.
pub trait CallStack {
    fn top_frame(&self) -> Frame;

    fn push_frame(&mut self, frame: Frame);

    fn pop_frame(&mut self) -> Option<Frame>;

    fn top(&self) -> Args {
        self.top_frame().args
    }

    fn top_kind(&self) -> Option<TagKind> {
        self.top_frame().kind
    }

    fn push(&mut self, kind: TagKind, args: Args) {
        self.push_frame(Frame {
            kind: Some(kind),
            args,
        });
    }

    fn pop(&mut self, kind: TagKind) -> Result<Args, InvalidDocument> {
        if self.top_kind().is_none() {
            return Err(InvalidDocument);
        }
        popped(self.pop_frame(), kind)
    }
}

#[derive(Debug, Clone)]
pub struct PrintStack(Vec<Frame>);

impl PrintStack {
    pub fn new() -> Self {
        Self(vec![ROOT_FRAME])
    }
}

impl CallStack for PrintStack {
    fn top_frame(&self) -> Frame {
        self.0.last().copied().unwrap_or(ROOT_FRAME)
    }

    fn push_frame(&mut self, frame: Frame) {
        self.0.push(frame);
    }

    fn pop_frame(&mut self) -> Option<Frame> {
        self.0.pop()
    }
}

// La pila con que se mide: lo que apila queda aparte y lo que desapila de más sale de una vista de
// la pila de verdad, que no se toca.
#[derive(Debug)]
pub struct FitsStack<'print> {
    own: Vec<Frame>,
    rest: &'print [Frame],
}

impl<'print> FitsStack<'print> {
    pub fn new(print_stack: &'print PrintStack) -> Self {
        Self {
            own: Vec::new(),
            rest: &print_stack.0,
        }
    }
}

impl CallStack for FitsStack<'_> {
    fn top_frame(&self) -> Frame {
        self.own
            .last()
            .or_else(|| self.rest.last())
            .copied()
            .unwrap_or(ROOT_FRAME)
    }

    fn push_frame(&mut self, frame: Frame) {
        self.own.push(frame);
    }

    fn pop_frame(&mut self) -> Option<Frame> {
        self.own.pop().or_else(|| {
            let (last, before) = self.rest.split_last()?;
            self.rest = before;
            Some(*last)
        })
    }
}
