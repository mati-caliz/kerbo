const OPENING_LENGTH: usize = 2;
const CLOSING: &str = "*/";
const CLOSING_AFTER_STARS: &str = " */";
const STAR: char = '*';
const MAX_TRIMMED_BLANKS: usize = 3;

// Port de `stripCommonPrefix` de go/printer: quita de las líneas de un `/* */` el prefijo común
// para volver a indentarlas con la indentación del código que lo rodea.
pub fn strip_common_prefix(lines: &mut [String]) {
    if lines.len() <= 1 {
        return;
    }
    let mut prefix = inner_prefix(lines);
    let line_of_stars = if let Some((before, _)) = prefix.split_once(STAR) {
        prefix = before.strip_suffix(' ').unwrap_or(before).to_owned();
        true
    } else {
        let first = lines.first().map(String::as_str).unwrap_or_default();
        prefix = without_opening_whitespace(&prefix, first);
        false
    };
    if let Some(last) = lines.last_mut() {
        let before = last
            .split_once(CLOSING)
            .map_or(last.as_str(), |(before, _)| before);
        if is_blank(before) {
            let closing = if line_of_stars {
                CLOSING_AFTER_STARS
            } else {
                CLOSING
            };
            *last = format!("{prefix}{closing}");
        } else {
            prefix = common_prefix(&prefix, last).to_owned();
        }
    }
    for line in lines.iter_mut().skip(1) {
        if !line.is_empty() {
            *line = line.get(prefix.len()..).unwrap_or_default().to_owned();
        }
    }
}

fn inner_prefix(lines: &mut [String]) -> String {
    let mut prefix: Option<String> = None;
    let count = lines.len();
    if count > 2 {
        for line in lines.iter_mut().take(count.saturating_sub(1)).skip(1) {
            if is_blank(line) {
                line.clear();
                continue;
            }
            let current = prefix.as_deref().unwrap_or(line);
            prefix = Some(common_prefix(current, line).to_owned());
        }
    }
    prefix.unwrap_or_else(|| {
        let last = lines.last().map(String::as_str).unwrap_or_default();
        common_prefix(last, last).to_owned()
    })
}

fn without_opening_whitespace(prefix: &str, first: &str) -> String {
    let after_opening = first.get(OPENING_LENGTH..).unwrap_or_default();
    if is_blank(after_opening) {
        let bytes = prefix.as_bytes();
        let mut end = bytes.len();
        let mut trimmed = 0;
        while trimmed < MAX_TRIMMED_BLANKS && end > 0 && bytes.get(end.saturating_sub(1)) == Some(&b' ') {
            end = end.saturating_sub(1);
            trimmed = trimmed.saturating_add(1);
        }
        if end == bytes.len() && end > 0 && bytes.get(end.saturating_sub(1)) == Some(&b'\t') {
            end = end.saturating_sub(1);
        }
        return prefix.get(..end).unwrap_or_default().to_owned();
    }
    let leading: String = after_opening
        .chars()
        .take_while(|character| *character <= ' ')
        .collect();
    let suffix = if leading.starts_with('\t') {
        leading
    } else {
        format!("  {leading}")
    };
    prefix.strip_suffix(suffix.as_str()).unwrap_or(prefix).to_owned()
}

fn is_blank(text: &str) -> bool {
    text.bytes().all(|byte| byte <= b' ')
}

fn common_prefix<'text>(first: &'text str, second: &str) -> &'text str {
    let length = first
        .bytes()
        .zip(second.bytes())
        .take_while(|(left, right)| left == right && (*left <= b' ' || *left == b'*'))
        .count();
    first.get(..length).unwrap_or_default()
}
