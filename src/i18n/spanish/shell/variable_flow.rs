use crate::i18n::catalog::RuleText;
use crate::rules::ShellRule;

// Chequeos de ShellCheck sobre el flujo de variables: nombres que nada asigna y cambios que se pierden en un subshell.
pub const VARIABLE_FLOW_TEXTS: [(ShellRule, RuleText); 4] = [
    (
        ShellRule::Sc2030,
        RuleText::new(
            "Asignación que queda adentro de un subshell (SC2030 de ShellCheck)",
            "asigná afuera del subshell, o imprimí el valor y capturalo",
            "Un subshell tiene su propia copia de las variables, así que el cambio se pierde al terminar.",
            "(x=1); echo \"$x\" muestra el valor de antes.",
        ),
    ),
    (
        ShellRule::Sc2031,
        RuleText::new(
            "Variable leída después de cambiarla en un subshell (SC2031 de ShellCheck)",
            "asigná afuera del subshell, o imprimí el valor y capturalo",
            "El cambio hecho en el subshell no se ve acá.",
            "cat f | while read -r x; do n=$x; done; echo \"$n\" no ve n.",
        ),
    ),
    (
        ShellRule::Sc2153,
        RuleText::new(
            "Variable posiblemente mal escrita: nada la asigna y hay una parecida (SC2153 de ShellCheck)",
            "revisá cómo se escribe, o asignala",
            "Una letra mal deja la variable vacía sin ningún error.",
            "REMOTE=x; echo \"$REMOTES\" lee una variable vacía.",
        ),
    ),
    (
        ShellRule::Sc2154,
        RuleText::new(
            "Variable leída y nunca asignada (SC2154 de ShellCheck)",
            "asignala, usá ${x:-valor}, o silenciala con una directiva si viene de afuera",
            "Si nada la asigna, el script corre con un valor vacío.",
            "echo \"$nombre\" sin un nombre= antes.",
        ),
    ),
];
