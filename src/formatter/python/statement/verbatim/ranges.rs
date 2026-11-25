use super::{is_off_in, is_on_in};
use crate::formatter::python::comments::SourceComment;

pub enum Range<'a> {
    Starts {
        formatted: &'a [SourceComment],
        off: &'a SourceComment,
    },
    Ends {
        suppressed: &'a [SourceComment],
        on: &'a SourceComment,
        formatted: &'a [SourceComment],
        off: Option<&'a SourceComment>,
    },
    Suppressed(&'a [SourceComment]),
    Formatted,
}

// `CommentRangeIter` de ruff: parte los comentarios de una sentencia en lo que se formatea y lo
// que queda apagado entre un `fmt: off` y el `fmt: on` que sigue.
pub struct RangeIter<'a> {
    comments: &'a [SourceComment],
    source: &'a str,
    in_suppression: bool,
}

impl<'a> RangeIter<'a> {
    pub const fn in_suppression(comments: &'a [SourceComment], source: &'a str) -> Self {
        Self {
            comments,
            source,
            in_suppression: true,
        }
    }

    pub const fn outside_suppression(comments: &'a [SourceComment], source: &'a str) -> Self {
        Self {
            comments,
            source,
            in_suppression: false,
        }
    }

    pub const fn remaining(&self) -> &'a [SourceComment] {
        self.comments
    }

    fn next_inside(&mut self) -> Range<'a> {
        let Some(on_position) = self
            .comments
            .iter()
            .position(|comment| is_on_in(self.source, comment))
        else {
            return Range::Suppressed(std::mem::take(&mut self.comments));
        };
        let (suppressed, from_on) = self.comments.split_at(on_position);
        let Some((on, rest)) = from_on.split_first() else {
            return Range::Suppressed(std::mem::take(&mut self.comments));
        };
        let off_position = rest.iter().position(|comment| is_off_in(self.source, comment));
        let next_off = off_position.and_then(|position| {
            let (formatted, from_off) = rest.split_at(position);
            from_off.split_first().map(|(off, after)| (formatted, off, after))
        });
        let (formatted, off) = if let Some((formatted, off, after)) = next_off {
            self.comments = after;
            (formatted, Some(off))
        } else {
            self.in_suppression = false;
            self.comments = &[];
            (rest, None)
        };
        Range::Ends {
            suppressed,
            on,
            formatted,
            off,
        }
    }

    fn next_outside(&mut self) -> Range<'a> {
        let Some(off_position) = self
            .comments
            .iter()
            .position(|comment| is_off_in(self.source, comment))
        else {
            return Range::Formatted;
        };
        let (formatted, from_off) = self.comments.split_at(off_position);
        let Some((off, after)) = from_off.split_first() else {
            return Range::Formatted;
        };
        self.in_suppression = true;
        self.comments = after;
        Range::Starts { formatted, off }
    }
}

impl<'a> Iterator for RangeIter<'a> {
    type Item = Range<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.comments.is_empty() {
            return None;
        }
        Some(if self.in_suppression {
            self.next_inside()
        } else {
            self.next_outside()
        })
    }
}
