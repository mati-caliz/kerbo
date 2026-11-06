use crate::i18n::catalog::RuleText;
use crate::rules::ShellRule;

// Avisos que ShellCheck da mientras parsea, y los últimos chequeos por defecto.
pub const SHELL_NOTES_TEXTS: [(ShellRule, RuleText); 4] = [
    (
        ShellRule::Sc1003,
        RuleText::new(
            "Barra antes de la comilla simple que cierra (SC1003 de ShellCheck)",
            "escribí 'it'\\''s' para incluir una comilla simple",
            "Adentro de comillas simples la barra es literal: la comilla cierra el texto.",
            "'it\\'s' se vuelve 'it'\\''s'.",
        ),
    ),
    (
        ShellRule::Sc1007,
        RuleText::new(
            "Espacio después del = en una asignación (SC1007 de ShellCheck)",
            "sacá el espacio, o usá var='' para un valor vacío",
            "La variable queda vacía y la palabra siguiente corre como comando.",
            "a= b se vuelve a=b.",
        ),
    ),
    (
        ShellRule::Sc2251,
        RuleText::new(
            "! que se saltea set -e (SC2251 de ShellCheck)",
            "usá && exit 1, o asegurate de mirar $?",
            "Un comando negado nunca corta el script con set -e.",
            "! true se vuelve true && exit 1.",
        ),
    ),
    (
        ShellRule::Sc2316,
        RuleText::new(
            "Comando de declaración usado como argumento de otro (SC2316 de ShellCheck)",
            "usá un comando aparte, o la opción de declare",
            "Declara una variable que se llama local o readonly.",
            "local readonly a se vuelve local -r a.",
        ),
    ),
];
