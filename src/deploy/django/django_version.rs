use std::fs;
use std::path::Path;

const PACKAGE: &str = "django";
const LOCK_FILES: [&str; 2] = ["uv.lock", "poetry.lock"];
const REQUIREMENT_FILES: [&str; 2] = ["requirements.txt", "requirements"];
const MANIFEST: &str = "pyproject.toml";
const REPOSITORY_MARKER: &str = ".git";
const LOCK_NAME_PREFIX: &str = "name = \"";
const LOCK_VERSION_PREFIX: &str = "version = \"";
const TEXT_EXTENSIONS: [&str; 2] = ["txt", "in"];
const VERSION_OPERATORS: [&str; 5] = ["==", "~=", ">=", "^", "~"];
const QUOTE: char = '"';
const QUOTES: [char; 2] = ['"', '\''];
const NAME_CHARACTERS: [char; 3] = ['-', '_', '.'];

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Version {
    pub major: u32,
    pub minor: u32,
}

impl Version {
    pub const fn new(major: u32, minor: u32) -> Self {
        Self { major, minor }
    }
}

pub fn project_version(apps_root: &Path) -> Option<Version> {
    for directory in apps_root.ancestors() {
        if let Some(version) = declared_in(directory) {
            return Some(version);
        }
        if directory.join(REPOSITORY_MARKER).exists() {
            return None;
        }
    }
    None
}

fn declared_in(directory: &Path) -> Option<Version> {
    LOCK_FILES
        .iter()
        .filter_map(|name| fs::read_to_string(directory.join(name)).ok())
        .find_map(|source| locked(&source))
        .or_else(|| {
            requirement_sources(directory)
                .iter()
                .find_map(|source| required(source))
        })
        .or_else(|| {
            fs::read_to_string(directory.join(MANIFEST))
                .ok()
                .and_then(|source| required(&source))
        })
}

fn requirement_sources(directory: &Path) -> Vec<String> {
    let mut sources = Vec::new();
    for name in REQUIREMENT_FILES {
        let path = directory.join(name);
        if path.is_dir() {
            let Ok(entries) = fs::read_dir(&path) else {
                continue;
            };
            let mut files: Vec<_> = entries
                .filter_map(Result::ok)
                .map(|entry| entry.path())
                .filter(|file| {
                    file.extension()
                        .and_then(|extension| extension.to_str())
                        .is_some_and(|extension| TEXT_EXTENSIONS.contains(&extension))
                })
                .collect();
            files.sort();
            sources.extend(files.iter().filter_map(|file| fs::read_to_string(file).ok()));
        } else if let Ok(source) = fs::read_to_string(&path) {
            sources.push(source);
        }
    }
    sources
}

fn locked(source: &str) -> Option<Version> {
    let mut lines = source.lines();
    while let Some(line) = lines.next() {
        let is_django = line
            .strip_prefix(LOCK_NAME_PREFIX)
            .and_then(|rest| rest.strip_suffix(QUOTE))
            .is_some_and(|name| name.eq_ignore_ascii_case(PACKAGE));
        if is_django {
            return lines
                .next()
                .and_then(|next| next.strip_prefix(LOCK_VERSION_PREFIX))
                .and_then(parsed);
        }
    }
    None
}

fn required(source: &str) -> Option<Version> {
    source.lines().find_map(|line| {
        let unquoted = line.trim().trim_start_matches(QUOTES);
        let name = unquoted.get(..PACKAGE.len())?;
        let rest = &unquoted[PACKAGE.len()..];
        let continues_the_name = rest
            .chars()
            .next()
            .is_none_or(|character| character.is_alphanumeric() || NAME_CHARACTERS.contains(&character));
        if !name.eq_ignore_ascii_case(PACKAGE) || continues_the_name {
            return None;
        }
        let start = rest.char_indices().find_map(|(index, _)| {
            VERSION_OPERATORS
                .iter()
                .find(|operator| rest[index..].starts_with(*operator))
                .map(|operator| index + operator.len())
        })?;
        parsed(&rest[start..])
    })
}

fn parsed(text: &str) -> Option<Version> {
    let mut numbers = text
        .trim_start()
        .split(|character: char| !character.is_ascii_digit())
        .skip_while(|part| part.is_empty())
        .take(2)
        .map(str::parse::<u32>);
    let major = numbers.next()?.ok()?;
    let minor = numbers.next().and_then(Result::ok).unwrap_or_default();
    Some(Version::new(major, minor))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_locked_version() {
        let lock = "[[package]]
name = \"django-filter\"
version = \"25.1\"

[[package]]
name = \"django\"
version = \"5.2.19\"
";
        assert_eq!(locked(lock), Some(Version::new(5, 2)));
    }

    #[test]
    fn reads_the_lower_bound_of_a_requirement() {
        assert_eq!(required("Django==4.2.11\n"), Some(Version::new(4, 2)));
        assert_eq!(
            required("django-environ==0.11\ndjango[argon2]>=5.1,<6\n"),
            Some(Version::new(5, 1))
        );
        assert_eq!(
            required("dependencies = [\n  \"django~=6.0\",\n]\n"),
            Some(Version::new(6, 0))
        );
        assert_eq!(
            required("[tool.poetry.dependencies]\nDjango = {version = \"^3.2\"}\n"),
            Some(Version::new(3, 2))
        );
        assert_eq!(
            required("DJANGO_SETTINGS_MODULE = \"shop.settings\"\ndjango<6\n"),
            None
        );
    }

    #[test]
    fn without_project_files_the_version_is_unknown() {
        assert_eq!(project_version(Path::new("no-existe/apps")), None);
    }
}
