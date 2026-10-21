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
    const DAEMON_VARIABLE: &str = "KERBO_DAEMON";
    const DAEMON_OFF: &str = "off";
    const CACHE_VARIABLE: &str = "KERBO_CACHE_DIR";
    const STARTUP_POLL: std::time::Duration = std::time::Duration::from_millis(50);
    const STARTUP_ATTEMPTS: usize = 100;

    fn kerbo(arguments: &[&str], directory: &PathBuf) -> Output {
        kerbo_with_locale(arguments, directory, None)
    }

    fn kerbo_with_locale(arguments: &[&str], directory: &PathBuf, locale: Option<&str>) -> Output {
        let mut command = Command::new(BINARY);
        command.env(DAEMON_VARIABLE, DAEMON_OFF);
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

    fn kerbo_with_cache(arguments: &[&str], directory: &PathBuf, input: &str) -> Output {
        use std::io::Write;
        use std::process::Stdio;
        let mut command = Command::new(BINARY);
        for variable in LOCALE_VARIABLES {
            command.env_remove(variable);
        }
        let mut child = command
            .args(arguments)
            .current_dir(directory)
            .env(CACHE_VARIABLE, directory.join("cache"))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("el binario corre");
        child
            .stdin
            .take()
            .expect("hay entrada")
            .write_all(input.as_bytes())
            .expect("se escribe el evento");
        child.wait_with_output().expect("el binario termina")
    }

    #[test]
    fn the_hook_starts_the_daemon_and_the_next_one_goes_through_it() {
        let directory = project("daemon");
        let event = format!(
            "{{\"tool_input\":{{\"file_path\":{:?}}}}}",
            directory.join("many.go").display().to_string()
        );
        let first = kerbo_with_cache(&["hook"], &directory, &event);
        assert_eq!(first.status.code(), Some(2));
        let running = (0..STARTUP_ATTEMPTS).any(|_| {
            std::thread::sleep(STARTUP_POLL);
            let status = kerbo_with_cache(&["daemon", "status"], &directory, "");
            String::from_utf8_lossy(&status.stderr).contains("está corriendo")
        });
        assert!(running, "el hook no dejó el demonio corriendo");
        let second = kerbo_with_cache(&["hook"], &directory, &event);
        assert_eq!(second.status.code(), Some(2));
        assert_eq!(second.stderr, first.stderr);
        let stopped = kerbo_with_cache(&["daemon", "stop"], &directory, "");
        assert!(String::from_utf8_lossy(&stopped.stderr).contains("demonio apagado"));
        fs::remove_dir_all(directory).expect("se limpia");
    }
}
