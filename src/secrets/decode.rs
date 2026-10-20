mod codecs;

use std::ops::Range;

use codecs::{Encoding, encodings_pattern};

const NEWLINE: u8 = b'\n';

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
struct Span {
    start: i64,
    end: i64,
}

impl Span {
    fn from_range(range: &Range<usize>) -> Self {
        Self {
            start: i64::try_from(range.start).unwrap_or(i64::MAX),
            end: i64::try_from(range.end).unwrap_or(i64::MAX),
        }
    }

    fn range(self) -> Range<usize> {
        let bound = |value: i64| usize::try_from(value).unwrap_or_default();
        bound(self.start)..bound(self.end)
    }

    const fn overlaps(self, other: Self) -> bool {
        other.start <= self.end && other.end >= self.start
    }

    const fn contains(self, other: Self) -> bool {
        self.start <= other.start && other.end <= self.end
    }

    fn merge(self, other: Self) -> Self {
        Self {
            start: self.start.min(other.start),
            end: self.end.max(other.end),
        }
    }

    const fn add(self, other: Self) -> Self {
        Self {
            start: self.start.saturating_add(other.start),
            end: self.end.saturating_add(other.end),
        }
    }

    fn overflow(self, other: Self) -> Self {
        let merged = self.merge(other);
        Self {
            start: merged.start.saturating_sub(self.start),
            end: merged.end.saturating_sub(self.end),
        }
    }
}

#[derive(Debug)]
pub struct Segment {
    original: Span,
    encoded: Span,
    decoded: Span,
}

#[derive(Debug, Default)]
pub struct Decoder {
    layers: Vec<Vec<Segment>>,
}

impl Decoder {
    pub fn latest(&self) -> Option<&[Segment]> {
        self.layers.last().map(Vec::as_slice)
    }

    pub fn decode(&mut self, text: &[u8]) -> Option<Vec<u8>> {
        let below = self.layers.len().checked_sub(1);
        let mut segments = Vec::new();
        let mut shift: i64 = 0;
        let mut decoded_values = Vec::new();
        for (encoding, range) in encoding_matches(text) {
            let decoded = encoding.decode(text.get(range.clone())?);
            if decoded.is_empty() {
                continue;
            }
            let encoded = Span::from_range(&range);
            let length = i64::try_from(decoded.len()).ok()?;
            let start = encoded.start.saturating_add(shift);
            segments.push(Segment {
                original: self.trace_layer(below, encoded),
                encoded,
                decoded: Span {
                    start,
                    end: start.saturating_add(length),
                },
            });
            shift = shift.saturating_add(length.saturating_sub(encoded.end.saturating_sub(encoded.start)));
            decoded_values.push(decoded);
        }
        if segments.is_empty() {
            return None;
        }
        let mut result = Vec::with_capacity(text.len());
        let mut copied = 0;
        for (segment, decoded) in segments.iter().zip(&decoded_values) {
            let range = segment.encoded.range();
            result.extend_from_slice(text.get(copied..range.start).unwrap_or_default());
            result.extend_from_slice(decoded);
            copied = range.end;
        }
        result.extend_from_slice(text.get(copied..).unwrap_or_default());
        self.layers.push(segments);
        Some(result)
    }

    pub fn overlapping(&self, span: &Range<usize>) -> Vec<&Segment> {
        let target = Span::from_range(span);
        self.latest()
            .unwrap_or_default()
            .iter()
            .filter(|segment| segment.decoded.overlaps(target))
            .collect()
    }

    pub fn original(&self, overlapping: &[&Segment], span: &Range<usize>) -> Range<usize> {
        let below = self.layers.len().checked_sub(2);
        self.trace(overlapping.iter().copied(), below, Span::from_range(span))
            .range()
    }

    fn trace_layer(&self, layer: Option<usize>, span: Span) -> Span {
        let Some(segments) = layer.and_then(|layer| self.layers.get(layer)) else {
            return span;
        };
        self.trace(
            segments.iter(),
            layer.and_then(|layer| layer.checked_sub(1)),
            span,
        )
    }

    fn trace<'segment>(
        &'segment self,
        segments: impl Iterator<Item = &'segment Segment>,
        below: Option<usize>,
        span: Span,
    ) -> Span {
        let mut encoded: Option<Span> = None;
        for segment in segments.filter(|segment| segment.decoded.overlaps(span)) {
            if segment.decoded.contains(span) {
                return segment.original;
            }
            let widened = segment.encoded.add(segment.decoded.overflow(span));
            encoded = Some(encoded.map_or(widened, |known| known.merge(widened)));
        }
        encoded
            .filter(|encoded| encoded.end != 0)
            .map_or(span, |encoded| self.trace_layer(below, encoded))
    }
}

pub fn current_line(overlapping: &[&Segment], text: &[u8]) -> Range<usize> {
    let decoded = overlapping
        .iter()
        .map(|segment| segment.decoded)
        .reduce(Span::merge)
        .unwrap_or_default()
        .range();
    let start = text
        .get(..=decoded.start.min(text.len().saturating_sub(1)))
        .and_then(|before| before.iter().rposition(|&byte| byte == NEWLINE))
        .unwrap_or(0);
    let end = text
        .get(decoded.end..)
        .and_then(|after| after.iter().position(|&byte| byte == NEWLINE))
        .map_or(text.len(), |offset| decoded.end.saturating_add(offset));
    start..end
}

fn encoding_matches(text: &[u8]) -> Vec<(Encoding, Range<usize>)> {
    let Some(pattern) = encodings_pattern() else {
        return Vec::new();
    };
    let all: Vec<(Encoding, Range<usize>)> = pattern
        .captures_iter(text)
        .filter_map(|captures| {
            Encoding::ALL.iter().enumerate().find_map(|(index, &encoding)| {
                captures
                    .get(index.saturating_add(1))
                    .map(|found| (encoding, found.range()))
            })
        })
        .collect();
    if all.len() == 1 {
        return all;
    }
    let beaten = |current: &(Encoding, Range<usize>), neighbour: Option<&(Encoding, Range<usize>)>| {
        neighbour.is_some_and(|(encoding, range)| {
            Span::from_range(&current.1).overlaps(Span::from_range(range))
                && encoding.precedence() > current.0.precedence()
        })
    };
    all.iter()
        .enumerate()
        .filter(|(index, current)| {
            let previous = index.checked_sub(1).and_then(|previous| all.get(previous));
            !beaten(current, previous) && !beaten(current, all.get(index.saturating_add(1)))
        })
        .map(|(_, current)| current.clone())
        .collect()
}
