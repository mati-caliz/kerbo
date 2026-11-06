use crate::i18n::catalog::RuleText;
use crate::rules::ShellRule;

// Chequeos de ShellCheck que siguen el tipo de cada variable y el grafo de control.
pub const FLOW_KINDS_TEXTS: [(ShellRule, RuleText); 10] = [
    (
        ShellRule::Sc2089,
        RuleText::new(
            "Comillas o barras guardadas en una variable (SC2089 de ShellCheck)",
            "usá un arreglo, o funciones en sh",
            "Se toman literales cuando se expande la variable.",
            "a='--x=\"b c\"' se vuelve a=(--x=\"b c\").",
        ),
    ),
    (
        ShellRule::Sc2090,
        RuleText::new(
            "Expansión de una variable con comillas guardadas (SC2090 de ShellCheck)",
            "usá un arreglo, o funciones en sh",
            "Las comillas del valor no se respetan.",
            "cmd $a se vuelve cmd \"${a[@]}\".",
        ),
    ),
    (
        ShellRule::Sc2128,
        RuleText::new(
            "Arreglo expandido sin índice (SC2128 de ShellCheck)",
            "usá \"${a[@]}\" o un índice",
            "Sólo da el primer elemento.",
            "echo $a se vuelve echo \"${a[@]}\".",
        ),
    ),
    (
        ShellRule::Sc2178,
        RuleText::new(
            "Arreglo al que se le asigna un texto (SC2178 de ShellCheck)",
            "usá otra variable, o a=(valor)",
            "Sólo reemplaza el primer elemento.",
            "a=(1 2); a=x se vuelve a=(x).",
        ),
    ),
    (
        ShellRule::Sc2179,
        RuleText::new(
            "+= con un texto sobre un arreglo (SC2179 de ShellCheck)",
            "usá a+=(\"item\")",
            "Agrega el texto al primer elemento.",
            "a=(1); a+=x se vuelve a+=(x).",
        ),
    ),
    (
        ShellRule::Sc2218,
        RuleText::new(
            "Función definida recién más adelante (SC2218 de ShellCheck)",
            "subí la definición",
            "Cuando corre la llamada, la función todavía no existe.",
            "f; f() { :; } se vuelve f() { :; }; f.",
        ),
    ),
    (
        ShellRule::Sc2317,
        RuleText::new(
            "Comando que nunca puede correr (SC2317 de ShellCheck)",
            "sacalo, o corregí el flujo de antes",
            "Algo antes siempre termina el script o la función.",
            "exit; foo se vuelve exit.",
        ),
    ),
    (
        ShellRule::Sc2319,
        RuleText::new(
            "$? que se refiere a una condición (SC2319 de ShellCheck)",
            "guardalo en una variable antes de la condición",
            "La condición pisó el código del comando anterior.",
            "[ $? -eq 0 ] && a=$? necesita r=$? antes.",
        ),
    ),
    (
        ShellRule::Sc2320,
        RuleText::new(
            "$? que se refiere a echo o printf (SC2320 de ShellCheck)",
            "guardalo en una variable antes de imprimir",
            "La impresión pisó el código del comando anterior.",
            "echo a; [ $? -eq 0 ] necesita r=$? antes.",
        ),
    ),
    (
        ShellRule::Sc2329,
        RuleText::new(
            "Función que nunca se llama (SC2329 de ShellCheck)",
            "sacala, o fijate si se llama de forma indirecta",
            "Nada en el script llega hasta ella.",
            "f() { :; }; exit no tiene ninguna llamada a f.",
        ),
    ),
];
