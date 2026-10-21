const LINE_FEED: char = '\n';
const MAX_EDIT_DISTANCE: usize = 1_000;
const REMOVED: &str = "-";
const ADDED: &str = "+";
const OLD_SIDE: &str = "--- a/";
const NEW_SIDE: &str = "+++ b/";
const SINGLE_LINE: usize = 1;
const NEWLINE: &str = "\n";
const NO_NEWLINE_AT_END: &str = "\\ No newline at end of file";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Step {
    Keep,
    Remove,
    Add,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Cursor {
    old: usize,
    new: usize,
}

pub fn file_patch(path: &str, old: &str, new: &str) -> Option<String> {
    if old == new {
        return None;
    }
    let old_lines: Vec<&str> = old.split_inclusive(LINE_FEED).collect();
    let new_lines: Vec<&str> = new.split_inclusive(LINE_FEED).collect();
    let steps = shortest_steps(&old_lines, &new_lines).unwrap_or_else(|| whole_steps(&old_lines, &new_lines));
    let mut output = vec![format!("{OLD_SIDE}{path}"), format!("{NEW_SIDE}{path}")];
    output.extend(hunks(&steps, &old_lines, &new_lines));
    output.push(String::new());
    Some(output.join(NEWLINE))
}

fn hunks(steps: &[Step], old: &[&str], new: &[&str]) -> Vec<String> {
    let mut output = Vec::new();
    let mut cursor = Cursor { old: 0, new: 0 };
    let mut index = 0;
    while let Some(&step) = steps.get(index) {
        if step == Step::Keep {
            cursor.old = cursor.old.saturating_add(1);
            cursor.new = cursor.new.saturating_add(1);
            index = index.saturating_add(1);
            continue;
        }
        let changed = steps
            .get(index..)
            .unwrap_or_default()
            .iter()
            .take_while(|candidate| **candidate != Step::Keep)
            .count();
        let block = steps
            .get(index..index.saturating_add(changed))
            .unwrap_or_default();
        let removed = block
            .iter()
            .filter(|candidate| **candidate == Step::Remove)
            .count();
        let added = changed.saturating_sub(removed);
        output.push(format!(
            "@@ -{} +{} @@",
            range(cursor.old, removed),
            range(cursor.new, added)
        ));
        output.extend(shown(old, cursor.old, removed, REMOVED));
        output.extend(shown(new, cursor.new, added, ADDED));
        cursor.old = cursor.old.saturating_add(removed);
        cursor.new = cursor.new.saturating_add(added);
        index = index.saturating_add(changed);
    }
    output
}

// Cada línea conserva su `\r` (en un archivo con CRLF `git apply` lo necesita) y la última sin salto
// lleva la marca de diff, igual que `git diff`.
fn shown(lines: &[&str], start: usize, count: usize, sign: &str) -> Vec<String> {
    let mut output = Vec::new();
    for line in lines.iter().skip(start).take(count) {
        output.push([sign, line.strip_suffix(LINE_FEED).unwrap_or(line)].concat());
        if !line.ends_with(LINE_FEED) {
            output.push(NO_NEWLINE_AT_END.to_owned());
        }
    }
    output
}

fn range(before: usize, count: usize) -> String {
    let start = if count == 0 {
        before
    } else {
        before.saturating_add(1)
    };
    if count == SINGLE_LINE {
        start.to_string()
    } else {
        format!("{start},{count}")
    }
}

fn whole_steps(old: &[&str], new: &[&str]) -> Vec<Step> {
    let prefix = old
        .iter()
        .zip(new)
        .take_while(|(left, right)| left == right)
        .count();
    let suffix = old
        .iter()
        .rev()
        .zip(new.iter().rev())
        .take(old.len().min(new.len()).saturating_sub(prefix))
        .take_while(|(left, right)| left == right)
        .count();
    let mut steps = vec![Step::Keep; prefix];
    steps.extend(vec![Step::Remove; old.len().saturating_sub(prefix + suffix)]);
    steps.extend(vec![Step::Add; new.len().saturating_sub(prefix + suffix)]);
    steps.extend(vec![Step::Keep; suffix]);
    steps
}

fn shortest_steps(old: &[&str], new: &[&str]) -> Option<Vec<Step>> {
    let trace = frontiers(old, new)?;
    let offset = trace.first().map_or(0, |frontier| frontier.len() / 2);
    let mut steps = Vec::new();
    let mut cursor = Cursor {
        old: old.len(),
        new: new.len(),
    };
    for (depth, frontier) in trace.iter().enumerate().rev() {
        let diagonal = (cursor.old + offset).checked_sub(cursor.new)?;
        if depth == 0 {
            steps.extend(vec![Step::Keep; cursor.old]);
            break;
        }
        let down = goes_down(frontier, diagonal, offset, depth);
        let previous_diagonal = if down {
            diagonal + 1
        } else {
            diagonal.checked_sub(1)?
        };
        let previous_old = *frontier.get(previous_diagonal)?;
        let previous_new = (previous_old + offset).checked_sub(previous_diagonal)?;
        while cursor.old > previous_old && cursor.new > previous_new {
            steps.push(Step::Keep);
            cursor.old -= 1;
            cursor.new -= 1;
        }
        steps.push(if down { Step::Add } else { Step::Remove });
        cursor = Cursor {
            old: previous_old,
            new: previous_new,
        };
    }
    steps.reverse();
    Some(steps)
}

fn frontiers(old: &[&str], new: &[&str]) -> Option<Vec<Vec<usize>>> {
    let limit = (old.len() + new.len()).min(MAX_EDIT_DISTANCE);
    let offset = limit + 1;
    let mut frontier = vec![0; 2 * offset + 1];
    let mut trace = Vec::new();
    for depth in 0..=limit {
        trace.push(frontier.clone());
        for diagonal in (offset - depth..=offset + depth).step_by(2) {
            let down = goes_down(&frontier, diagonal, offset, depth);
            let mut old_index = if down {
                *frontier.get(diagonal + 1)?
            } else {
                frontier.get(diagonal - 1)?.saturating_add(1)
            };
            let mut new_index = (old_index + offset).checked_sub(diagonal)?;
            while old
                .get(old_index)
                .is_some_and(|line| new.get(new_index) == Some(line))
            {
                old_index += 1;
                new_index += 1;
            }
            *frontier.get_mut(diagonal)? = old_index;
            if old_index >= old.len() && new_index >= new.len() {
                return Some(trace);
            }
        }
    }
    None
}

fn goes_down(frontier: &[usize], diagonal: usize, offset: usize, depth: usize) -> bool {
    diagonal + depth == offset
        || (diagonal != offset + depth
            && frontier.get(diagonal - 1).copied().unwrap_or_default()
                < frontier.get(diagonal + 1).copied().unwrap_or_default())
}
