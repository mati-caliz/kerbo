use std::collections::BTreeSet;

const SETTING: &str = "INSTALLED_APPS";
const MODULE_SEPARATOR: char = '.';
const APPS_MODULE: &str = "apps";
const CONFIG_SUFFIX: &str = "Config";
const CONCATENATION: char = '+';
const ASSIGNMENT: char = '=';
const ANNOTATION: char = ':';
const COMMENT: char = '#';
const QUOTES: [char; 2] = ['"', '\''];
const OPENERS: [char; 2] = ['[', '('];
const CLOSERS: [char; 2] = [']', ')'];
const MAXIMUM_INDIRECTION: usize = 8;
const NEWLINE: char = '\n';
const INLINE_WHITESPACE: [char; 2] = [' ', '\t'];
const LIST_ADDITIONS: [&str; 2] = [".append(", ".insert("];
const CALL_OPENER: char = '(';

pub fn installed_apps(sources: &[String]) -> Option<BTreeSet<String>> {
    let text = sources
        .iter()
        .flat_map(|source| source.lines())
        .map(without_comment)
        .collect::<Vec<_>>()
        .join("\n");
    let mut apps = BTreeSet::new();
    let mut found = false;
    for expression in assignments(&text, SETTING) {
        for module in modules(&text, expression, 0)? {
            apps.insert(app_directory(&module));
        }
        found = true;
    }
    apps.extend(added(&text).iter().map(|module| app_directory(module)));
    found.then_some(apps)
}

fn added(text: &str) -> Vec<String> {
    let mut modules = Vec::new();
    for method in LIST_ADDITIONS {
        let call = format!("{SETTING}{method}");
        for (start, _) in text.match_indices(&call) {
            if let Some((inside, _)) = bracketed(&text[start + call.len() - 1..], CALL_OPENER) {
                modules.extend(strings(inside));
            }
        }
    }
    modules
}

fn without_comment(line: &str) -> &str {
    let mut quote = None;
    for (index, character) in line.char_indices() {
        match quote {
            Some(open) if character == open => quote = None,
            None if QUOTES.contains(&character) => quote = Some(character),
            None if character == COMMENT => return &line[..index],
            Some(_) | None => {}
        }
    }
    line
}

fn skip_annotation(annotation: &str) -> &str {
    annotation
        .find(ASSIGNMENT)
        .map_or(annotation, |assignment| &annotation[assignment..])
}

fn assignments<'text>(text: &'text str, name: &str) -> Vec<&'text str> {
    let mut found = Vec::new();
    let mut offset = 0;
    while let Some(position) = text[offset..].find(name) {
        let start = offset + position;
        offset = start + name.len();
        let line_start = text[..start].rfind(NEWLINE).map_or(0, |newline| newline + 1);
        if !text[line_start..start].trim().is_empty() {
            continue;
        }
        let after = text[offset..].trim_start_matches(INLINE_WHITESPACE);
        let after = after.strip_prefix(ANNOTATION).map_or(after, skip_annotation);
        let operator = after.strip_prefix(CONCATENATION).unwrap_or(after);
        if let Some(expression) = operator
            .strip_prefix(ASSIGNMENT)
            .filter(|value| !value.starts_with(ASSIGNMENT))
        {
            found.push(expression);
        }
    }
    found
}

fn modules(text: &str, expression: &str, depth: usize) -> Option<Vec<String>> {
    if depth > MAXIMUM_INDIRECTION {
        return None;
    }
    let mut found = Vec::new();
    let mut rest = expression.trim_start();
    loop {
        if let Some(opener) = rest
            .chars()
            .next()
            .filter(|character| OPENERS.contains(character))
        {
            let (literal, after) = bracketed(rest, opener)?;
            found.extend(strings(literal));
            rest = after;
        } else {
            let length = rest
                .find(|character: char| !(character.is_alphanumeric() || character == '_'))
                .unwrap_or(rest.len());
            if length == 0 {
                return None;
            }
            let definitions = assignments(text, &rest[..length]);
            if definitions.is_empty() {
                return None;
            }
            for definition in definitions {
                found.extend(modules(text, definition, depth + 1)?);
            }
            rest = &rest[length..];
        }
        let trimmed = rest.trim_start_matches(INLINE_WHITESPACE);
        match trimmed.strip_prefix(CONCATENATION) {
            Some(next) => rest = next.trim_start(),
            None => return Some(found),
        }
    }
}

fn bracketed(text: &str, opener: char) -> Option<(&str, &str)> {
    let closer = *CLOSERS.get(OPENERS.iter().position(|candidate| *candidate == opener)?)?;
    let mut depth = 0_usize;
    let mut quote = None;
    for (index, character) in text.char_indices() {
        match quote {
            Some(open) if character == open => quote = None,
            None if QUOTES.contains(&character) => quote = Some(character),
            None if character == opener => depth += 1,
            None if character == closer => {
                depth -= 1;
                if depth == 0 {
                    return Some((&text[1..index], &text[index + 1..]));
                }
            }
            Some(_) | None => {}
        }
    }
    None
}

fn strings(literal: &str) -> Vec<String> {
    literal
        .split(',')
        .map(str::trim)
        .filter_map(|item| {
            let quote = item
                .chars()
                .next()
                .filter(|character| QUOTES.contains(character))?;
            item.strip_prefix(quote)?.strip_suffix(quote).map(str::to_owned)
        })
        .collect()
}

fn app_directory(module: &str) -> String {
    let segments: Vec<&str> = module.split(MODULE_SEPARATOR).collect();
    let package = match segments.as_slice() {
        [.., package, APPS_MODULE, config] if config.ends_with(CONFIG_SUFFIX) => package,
        [.., last] => last,
        [] => module,
    };
    (*package).to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn read(source: &str) -> Option<Vec<String>> {
        installed_apps(&[source.to_owned()]).map(|apps| apps.into_iter().collect())
    }

    #[test]
    fn reads_the_literal_list_and_later_additions() {
        let source = "INSTALLED_APPS = [\n    # Django\n    \"django.contrib.auth\",\n    'shop.orders',  # pedidos\n]\nif DEBUG:\n    INSTALLED_APPS += [\"debug_toolbar\"]\n";
        assert_eq!(
            read(source),
            Some(vec![
                "auth".to_owned(),
                "debug_toolbar".to_owned(),
                "orders".to_owned()
            ])
        );
    }

    #[test]
    fn follows_the_lists_that_are_concatenated() {
        let source = "DJANGO_APPS = (\"django.contrib.admin\",)\nLOCAL_APPS = [\"shop.users.apps.UsersConfig\"]\nINSTALLED_APPS = DJANGO_APPS + LOCAL_APPS + [\"extra\"]\n";
        assert_eq!(
            read(source),
            Some(vec!["admin".to_owned(), "extra".to_owned(), "users".to_owned()])
        );
    }

    #[test]
    fn joins_every_definition_of_a_concatenated_list() {
        let source = "EXTRA_APPS = [\"analytics\"]\nINSTALLED_APPS = [\"zerver\"]\nINSTALLED_APPS += EXTRA_APPS\nEXTRA_APPS = [\"zilencer\", \"corporate\"]\n";
        assert_eq!(
            read(source),
            Some(vec![
                "analytics".to_owned(),
                "corporate".to_owned(),
                "zerver".to_owned(),
                "zilencer".to_owned()
            ])
        );
    }

    #[test]
    fn counts_the_apps_appended_or_inserted_later() {
        let source = "INSTALLED_APPS = [\"cms\"]
if LEGACY:
    INSTALLED_APPS.insert(0, \"pages\")
else:
    INSTALLED_APPS.append(\"cms.customuser\")
";
        assert_eq!(
            read(source),
            Some(vec![
                "cms".to_owned(),
                "customuser".to_owned(),
                "pages".to_owned()
            ])
        );
    }

    #[test]
    fn reads_an_annotated_list() {
        let source = "INSTALLED_APPS: tuple[str, ...] = (
    \"django.contrib.auth\",
    \"sentry.users.apps.Config\",
)
";
        assert_eq!(read(source), Some(vec!["auth".to_owned(), "users".to_owned()]));
    }

    #[test]
    fn gives_up_when_the_list_is_computed() {
        assert_eq!(read("INSTALLED_APPS = get_apps()\n"), None);
        assert_eq!(read("DEBUG = True\n"), None);
        assert_eq!(read("if \"shop\" in INSTALLED_APPS == x:\n    pass\n"), None);
    }
}
