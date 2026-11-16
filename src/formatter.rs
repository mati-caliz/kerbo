mod go;

pub use go::format as format_go;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unparsable {
    SyntaxError { line: usize },
    ReadsUnlikeGo { line: usize },
    Unsupported { line: usize },
}
