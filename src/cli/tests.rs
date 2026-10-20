use super::{Command, command, try_parse};
use crate::i18n::Lang;

#[test]
fn every_language_builds_a_valid_command_line() {
    for lang in [Lang::Spanish, Lang::English] {
        command(lang).debug_assert();
    }
}

#[test]
fn the_help_follows_the_language() {
    let spanish = command(Lang::Spanish).render_long_help().to_string();
    let english = command(Lang::English).render_long_help().to_string();
    assert!(spanish.contains("Analiza los archivos y lista los problemas"));
    assert!(english.contains("Analyzes the files and lists the problems"));
    let mut check = command(Lang::English);
    let check_help = check
        .find_subcommand_mut("check")
        .expect("existe check")
        .render_long_help()
        .to_string();
    assert!(check_help.contains("RULE=VALUE") && check_help.contains("Output format"));
}

#[test]
fn invalid_values_are_explained_in_the_chosen_language() {
    let english = try_parse(Lang::English, ["kerbo", "check", "--max", "nope=1"]).expect_err("es inválido");
    assert!(english.to_string().contains("there is no rule `nope`"));
    let spanish = try_parse(Lang::Spanish, ["kerbo", "explain", "nope"]).expect_err("es inválido");
    assert!(spanish.to_string().contains("no existe la regla `nope`"));
    let parsed = try_parse(
        Lang::Spanish,
        ["kerbo", "explain", "--lang", "en", "magic-number"],
    )
    .expect("es válido");
    assert_eq!(parsed.lang, Some(Lang::English));
    assert!(matches!(parsed.command, Command::Explain { .. }));
}

#[test]
fn the_osv_lookup_is_opt_in_and_offline_on_request() {
    use crate::osv::{Ecosystem, OsvMode};
    let modes = |arguments: &[&str]| {
        let parsed = try_parse(Lang::Spanish, arguments.iter().copied()).expect("es válido");
        let Command::Check(check) = parsed.command else {
            panic!("se esperaba check");
        };
        (check.osv, check.scan.paths.len())
    };
    assert_eq!(modes(&["kerbo", "check"]), (None, 1));
    assert_eq!(
        modes(&["kerbo", "check", "--osv", "src"]),
        (Some(OsvMode::Online), 1)
    );
    assert_eq!(
        modes(&["kerbo", "check", "--osv=offline"]),
        (Some(OsvMode::Offline), 1)
    );
    let download = try_parse(Lang::English, ["kerbo", "osv-download", "npm", "PyPI"]).expect("es válido");
    assert!(matches!(
        download.command,
        Command::OsvDownload { ecosystems } if ecosystems == [Ecosystem::Npm, Ecosystem::PyPi]
    ));
}
