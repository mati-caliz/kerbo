mod go;

pub use go::format as format_go;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Unparsable;
