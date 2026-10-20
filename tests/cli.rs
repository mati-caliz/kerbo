#[cfg(test)]
mod binary {
    use std::fs;
    use std::path::PathBuf;
    use std::process::{Command, Output};

    const BINARY: &str = env!("CARGO_BIN_EXE_kerbo");

    fn project(name: &str) -> PathBuf {
        let directory = std::env::temp_dir().join(format!("kerbo-cli-{}-{name}", std::process::id()));
        if directory.exists() {
            fs::remove_dir_all(&directory).expect("se borra una corrida anterior");
        }
        fs::create_dir_all(&directory).expect("se crea la carpeta");
        fs::write(
            directory.join("many.go"),
            "package sample\n\nfunc Many(a, b, c, d, e int) {}\n",
        )
        .expect("se escribe el archivo");
        directory
    }

    const LOCALE_VARIABLES: [&str; 3] = ["LC_ALL", "LC_MESSAGES", "LANG"];

    fn kerbo(arguments: &[&str], directory: &PathBuf) -> Output {
        kerbo_with_locale(arguments, directory, None)
    }

    fn kerbo_with_locale(arguments: &[&str], directory: &PathBuf, locale: Option<&str>) -> Output {
        let mut command = Command::new(BINARY);
        for variable in LOCALE_VARIABLES {
            command.env_remove(variable);
        }
        if let Some(locale) = locale {
            command.env("LANG", locale);
        }
        command
            .args(arguments)
            .current_dir(directory)
            .output()
            .expect("el binario corre")
    }

    #[test]
    fn follows_the_locale_unless_the_flag_says_otherwise() {
        let directory = project("locale");
        let english = kerbo_with_locale(&["check"], &directory, Some("en_US.UTF-8"));
        assert!(String::from_utf8_lossy(&english.stderr).contains("1 file, 1 problem"));
        let help = kerbo_with_locale(&["--help"], &directory, Some("en_US.UTF-8"));
        assert!(String::from_utf8_lossy(&help.stdout).contains("Analyzes the files"));
        let flagged = kerbo_with_locale(&["check", "--lang", "es"], &directory, Some("en_US.UTF-8"));
        assert!(String::from_utf8_lossy(&flagged.stderr).contains("1 archivo, 1 problema"));
        let spanish_help = kerbo(&["--help"], &directory);
        assert!(String::from_utf8_lossy(&spanish_help.stdout).contains("Analiza los archivos"));
        fs::remove_dir_all(directory).expect("se limpia");
    }

    #[test]
    fn checks_a_project_from_its_own_directory() {
        let directory = project("check");
        let output = kerbo(&["check"], &directory);
        assert_eq!(output.status.code(), Some(1));
        assert!(String::from_utf8_lossy(&output.stdout).contains("parameter-count"));
        assert!(String::from_utf8_lossy(&output.stderr).contains("1 archivo, 1 problema"));
        fs::remove_dir_all(directory).expect("se limpia");
    }

    #[test]
    fn prints_the_version_and_rejects_unknown_commands() {
        let directory = project("version");
        let version = kerbo(&["--version"], &directory);
        assert!(String::from_utf8_lossy(&version.stdout).starts_with("kerbo "));
        assert_eq!(kerbo(&["nope"], &directory).status.code(), Some(2));
        fs::remove_dir_all(directory).expect("se limpia");
    }
}
