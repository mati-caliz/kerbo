// Lo que es una shell: si el programa que se corre es una, lo que va después de `-c` es un comando
// entero, y elegir lo que se le pasa es una inyección.
const SHELLS: &[&str] = &[
    "sh",
    "bash",
    "zsh",
    "dash",
    "ksh",
    "cmd",
    "cmd.exe",
    "powershell",
    "pwsh",
];
const SHELL_COMMAND_FLAGS: &[&str] = &["-c", "/c", "-Command"];
const PATH_SEPARATORS: &[char] = &['/', '\\'];
pub fn is_shell(program: &str) -> bool {
    let name = program.rsplit(PATH_SEPARATORS).next().unwrap_or(program);
    SHELLS.contains(&name)
}

pub fn is_shell_command_flag(written: &str) -> bool {
    SHELL_COMMAND_FLAGS.contains(&written)
}
