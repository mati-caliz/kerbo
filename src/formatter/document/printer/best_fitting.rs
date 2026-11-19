use super::queue::ElementQueue;
use super::stack::{CallStack, MeasureMode};
use super::{Frames, InvalidDocument, Printer};
use crate::formatter::document::element::{BestFittingMode, Element, GroupId, PrintMode, Start, TagKind};

const fn content_of(variant: &[Element]) -> Result<&[Element], InvalidDocument> {
    match variant.split_first() {
        Some((Element::Start(Start::BestFittingEntry), content)) => Ok(content),
        Some(_) | None => Err(InvalidDocument),
    }
}

// `best_fitting!` de ruff_formatter: imprime la primera variante que entra midiéndola plana, y si
// ninguna entra, la última, partida.
pub fn print<'document>(
    printer: &mut Printer<'document>,
    (variants, mode): (&'document [Box<[Element]>], BestFittingMode),
    frames: &mut Frames<'document>,
) -> Result<(), InvalidDocument> {
    let args = frames.stack.top();
    let (most_expanded, flatter) = variants.split_last().ok_or(InvalidDocument)?;
    if args.mode == PrintMode::Flat && printer.measured_group_fits {
        let most_flat = flatter.first().unwrap_or(most_expanded);
        frames.queue.extend_back(most_flat);
        return printer.print_entry(frames, args, TagKind::BestFittingEntry);
    }
    printer.measured_group_fits = true;
    let flat = args.with_mode(PrintMode::Flat);
    for variant in flatter {
        let content = content_of(variant)?;
        frames.queue.extend_back(content);
        frames
            .stack
            .push(TagKind::BestFittingEntry, flat.with_measure(mode.into()));
        let variant_fits = printer.fits(frames)?;
        frames.stack.pop(TagKind::BestFittingEntry)?;
        if !content.is_empty() {
            frames.queue.pop_slice();
        }
        if variant_fits {
            frames.queue.extend_back(variant);
            return printer.print_entry(frames, flat, TagKind::BestFittingEntry);
        }
    }
    frames.queue.extend_back(most_expanded);
    printer.print_entry(
        frames,
        args.with_mode(PrintMode::Expanded),
        TagKind::BestFittingEntry,
    )
}

// `best_fit_parenthesize` de ruff_formatter: si el contenido no entra en la línea pero entra entre
// paréntesis en las líneas siguientes, el impresor agrega los paréntesis y la sangría; si tampoco
// entra así, queda sin paréntesis.
pub fn start_parenthesize<'document>(
    printer: &mut Printer<'document>,
    id: Option<GroupId>,
    frames: &mut Frames<'document>,
) -> Result<(), InvalidDocument> {
    let args = frames.stack.top();
    let sentinels = printer.sentinels;
    let mut mode = printer.flat_group_print_mode(TagKind::BestFitParenthesize, id, frames)?;
    if mode == PrintMode::Expanded {
        printer.remember(id, PrintMode::Expanded);
        frames.stack.push(
            TagKind::BestFitParenthesize,
            args.with_measure(MeasureMode::AllLines),
        );
        frames.queue.extend_back(&sentinels.open_parenthesis);
        let fits_parenthesized = printer.fits(frames)?;
        frames.queue.pop_slice();
        frames.stack.pop(TagKind::BestFitParenthesize)?;
        if !fits_parenthesized {
            mode = PrintMode::Flat;
        }
    }
    printer.remember(id, mode);
    if mode == PrintMode::Expanded {
        frames.queue.extend_back(&sentinels.open_parenthesis);
    }
    frames
        .stack
        .push(TagKind::BestFitParenthesize, args.with_mode(mode));
    Ok(())
}
