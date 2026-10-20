use super::{Command, parse, words};

fn names(script: &str) -> Vec<String> {
    parse(script)
        .commands
        .into_iter()
        .map(|command| command.name)
        .collect()
}

#[test]
fn splits_commands_on_operators_and_skips_shell_keywords() {
    assert_eq!(
        names("if [ -f a ]; then apt-get update && rm -rf x || true; fi; for x in a b; do echo $x; done"),
        vec!["apt-get", "rm", "true", "echo"]
    );
    assert_eq!(
        names("DEBIAN_FRONTEND=noninteractive apt-get install -y curl"),
        vec!["apt-get"]
    );
    assert_eq!(names("A=1 B=2"), Vec::<String>::new());
}

#[test]
fn reads_quotes_substitutions_and_redirections() {
    let script = parse(
        "echo \"$(id -u)\" 'a b' >/dev/null 2>&1 && cat <<EOF > out\nnot a command\nEOF\nls `pwd` | wc -l",
    );
    let found: Vec<(String, Vec<String>)> = script
        .commands
        .into_iter()
        .map(|command| (command.name, command.arguments))
        .collect();
    assert_eq!(
        found,
        vec![
            ("echo".to_owned(), vec!["${id-u}".to_owned(), "a b".to_owned()]),
            ("id".to_owned(), vec!["-u".to_owned()]),
            ("cat".to_owned(), Vec::new()),
            ("ls".to_owned(), vec!["${VAR}".to_owned()]),
            ("pwd".to_owned(), Vec::new()),
            ("wc".to_owned(), vec!["-l".to_owned()]),
        ]
    );
    assert!(parse("a | b").has_pipe);
    assert!(parse("echo $(a | b)").has_pipe);
    assert_eq!(
        parse("pip install $(cat x | sort)").commands[0].arguments[1],
        "${}"
    );
    assert!(!parse("a || b").has_pipe);
}

#[test]
fn reads_flags_like_hadolint() {
    let command = Command {
        name: "apt-get".to_owned(),
        arguments: [
            "-yq",
            "--no-install-recommends",
            "-t",
            "bookworm",
            "install",
            "curl",
            "--opt=x",
        ]
        .map(str::to_owned)
        .to_vec(),
    };
    assert!(command.has_flag("y") && command.has_flag("q") && command.has_flag("opt"));
    assert_eq!(command.flag_count("q"), 1);
    assert_eq!(
        command.arguments_without_flags(),
        vec!["bookworm", "install", "curl"]
    );
    assert_eq!(
        command.without_flag_values(&["t"]).arguments_without_flags(),
        vec!["install", "curl"]
    );
}

#[test]
fn keeps_variables_inside_words() {
    assert_eq!(words("A=$B C=\"d e\" F=${G}"), vec!["A=${B}", "C=d e", "F=${G}"]);
}
