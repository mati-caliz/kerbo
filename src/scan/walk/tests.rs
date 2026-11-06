use std::path::Path;

use super::is_analyzed;
use crate::language::Language;

fn python_is_analyzed(relative: &str) -> bool {
    is_analyzed(
        Path::new("root"),
        &Path::new("root").join(relative),
        Some(Language::Python),
    )
}

#[test]
fn python_walks_like_ruff_hidden_included_and_its_default_excludes_out() {
    assert!(python_is_analyzed(".ci/check.py"));
    assert!(python_is_analyzed(".hidden.py"));
    assert!(python_is_analyzed("src/app.py"));
    assert!(!python_is_analyzed("venv/lib/site.py"));
    assert!(!python_is_analyzed("pkg/dist/built.py"));
    assert!(!python_is_analyzed(".tox/lib/site.py"));
}

#[test]
fn the_other_languages_skip_what_is_hidden_except_the_listed_folders() {
    let analyzed = |relative: &str| {
        is_analyzed(
            Path::new("root"),
            &Path::new("root").join(relative),
            Some(Language::Go),
        )
    };
    assert!(analyzed("cmd/main.go"));
    assert!(analyzed(".github/tools/main.go"));
    assert!(!analyzed(".ci/main.go"));
    assert!(!analyzed(".hidden.go"));
    assert!(!analyzed(".github/.hidden.go"));
}
