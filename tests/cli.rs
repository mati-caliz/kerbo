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

    fn kerbo(arguments: &[&str], directory: &PathBuf) -> Output {
        Command::new(BINARY)
            .args(arguments)
            .current_dir(directory)
            .output()
            .expect("el binario corre")
    }

    #[test]
    fn checks_a_project_from_its_own_directory() {
        let directory = project("check");
        let output = kerbo(&["check"], &directory);
        assert_eq!(output.status.code(), Some(1));
        assert!(String::from_utf8_lossy(&output.stdout).contains("parameter-count"));
        assert!(String::from_utf8_lossy(&output.stderr).contains("1 archivos, 1 problemas"));
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
