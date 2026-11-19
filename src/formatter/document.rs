mod builder;
mod element;
mod expand;
mod inspect;
mod printer;

pub use builder::Builder;
pub use element::{
    BestFittingMode, Condition, Element, GroupId, LineMode, PrintMode, Start, TagKind, text_width,
};
pub use inspect::{remove_soft_line_breaks, will_break};
pub use printer::print;

#[cfg(test)]
mod tests;
