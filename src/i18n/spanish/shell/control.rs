use crate::i18n::catalog::RuleText;
use crate::rules::ShellRule;

// Chequeos de Analytics.hs de ShellCheck sobre el flujo del script: loops, exec, break, funciones que se pasan a otro comando y el shebang.
pub const CONTROL_TEXTS: [(ShellRule, RuleText); 35] = [
    (
        ShellRule::Sc2000,
        RuleText::new(
            "echo a wc para medir una variable (SC2000 de ShellCheck)",
            "usá ${#variable}",
            "Una expansión la mide sin arrancar dos procesos.",
            "$(echo \"$a\" | wc -c) se vuelve ${#a}.",
        ),
    ),
    (
        ShellRule::Sc2014,
        RuleText::new(
            "Glob o sustitución en find -exec (SC2014 de ShellCheck)",
            "ponelo entre comillas, o corrélo con sh -c",
            "Se expande una vez antes de correr find, no por cada archivo.",
            "find . -exec echo * \\; se vuelve find . -exec echo '*' \\;.",
        ),
    ),
    (
        ShellRule::Sc2032,
        RuleText::new(
            "Función que un comando externo no va a poder correr (SC2032 de ShellCheck)",
            "usá un script aparte o sh -c",
            "Las funciones sólo existen en el shell actual.",
            "foo() { :; }; sudo foo necesita un script.",
        ),
    ),
    (
        ShellRule::Sc2033,
        RuleText::new(
            "Función pasada a un comando externo (SC2033 de ShellCheck)",
            "usá un script aparte o sh -c",
            "sudo, xargs y compañía corren programas: no ven las funciones del shell.",
            "foo() { :; }; sudo foo necesita un script.",
        ),
    ),
    (
        ShellRule::Sc2036,
        RuleText::new(
            "Asignación al principio de un pipeline (SC2036 de ShellCheck)",
            "usá a=$(b | c)",
            "La variable no recibe la salida del pipeline.",
            "a=b | c se vuelve a=$(b | c).",
        ),
    ),
    (
        ShellRule::Sc2041,
        RuleText::new(
            "for sobre un texto entre comillas simples (SC2041 de ShellCheck)",
            "usá $(..) para correr un comando",
            "Es un texto literal, así que el loop corre una sola vez.",
            "for f in 'ls' se vuelve for f in $(ls).",
        ),
    ),
    (
        ShellRule::Sc2042,
        RuleText::new(
            "Comas entre los valores de un for (SC2042 de ShellCheck)",
            "separá los valores con espacios",
            "Las comas quedan en el valor, y el loop corre una vez.",
            "for i in 1,2 se vuelve for i in 1 2.",
        ),
    ),
    (
        ShellRule::Sc2043,
        RuleText::new(
            "for que corre una sola vez (SC2043 de ShellCheck)",
            "corregí las comillas, o agregá el glob o la expansión que falta",
            "Un loop sobre un solo valor fijo probablemente perdió algo.",
            "for f in foo se vuelve for f in foo*.",
        ),
    ),
    (
        ShellRule::Sc2066,
        RuleText::new(
            "for sobre algo entre comillas que tenía que partirse (SC2066 de ShellCheck)",
            "sacá las comillas, o recorré un arreglo",
            "Con comillas no se parte: el loop corre una vez.",
            "for f in \"$(ls)\" se vuelve for f in *.",
        ),
    ),
    (
        ShellRule::Sc2067,
        RuleText::new(
            "find -exec sin su terminador (SC2067 de ShellCheck)",
            "terminalo con \\; o +",
            "Sin eso find falla: |, && y un ; sin comillas no terminan -exec.",
            "find . -exec cmd {} ; se vuelve find . -exec cmd {} \\;.",
        ),
    ),
    (
        ShellRule::Sc2083,
        RuleText::new(
            "Espacio después de ./ (SC2083 de ShellCheck)",
            "sacá el espacio",
            "./ es una carpeta, no el programa.",
            "./ foo se vuelve ./foo.",
        ),
    ),
    (
        ShellRule::Sc2087,
        RuleText::new(
            "Here document sin comillas pasado a ssh (SC2087 de ShellCheck)",
            "poné comillas en el delimitador",
            "Las expansiones pasan en el cliente, no en el servidor.",
            "ssh host << EOF se vuelve ssh host << 'EOF'.",
        ),
    ),
    (
        ShellRule::Sc2093,
        RuleText::new(
            "Los comandos después de exec no corren (SC2093 de ShellCheck)",
            "sacá exec si el script tiene que seguir",
            "exec reemplaza al shell por el comando.",
            "exec foo; bar se vuelve foo; bar.",
        ),
    ),
    (
        ShellRule::Sc2095,
        RuleText::new(
            "Comando que se come la entrada de un while read (SC2095 de ShellCheck)",
            "agregá la opción que lo desconecta de stdin, o redirigilo",
            "ssh y compañía leen el stdin del loop, que termina en la primera línea.",
            "ssh host cmd se vuelve ssh -n host cmd.",
        ),
    ),
    (
        ShellRule::Sc2096,
        RuleText::new(
            "Shebang con más de un parámetro (SC2096 de ShellCheck)",
            "dejá un solo parámetro, o usá env -S",
            "La mayoría de los sistemas lo pasa todo como un solo argumento.",
            "#!/bin/bash -e -x se vuelve #!/bin/bash -ex.",
        ),
    ),
    (
        ShellRule::Sc2097,
        RuleText::new(
            "Asignación delante de un comando que sólo ve el proceso (SC2097 de ShellCheck)",
            "asigná en una línea aparte",
            "La expansión del mismo comando corre antes de que valga la asignación.",
            "a=1 echo $a se vuelve a=1; echo \"$a\".",
        ),
    ),
    (
        ShellRule::Sc2098,
        RuleText::new(
            "Expansión que no ve la asignación delante del comando (SC2098 de ShellCheck)",
            "asigná en una línea aparte",
            "La expansión pasa antes de que valga la asignación.",
            "a=1 echo $a se vuelve a=1; echo \"$a\".",
        ),
    ),
    (
        ShellRule::Sc2101,
        RuleText::new(
            "Clase de caracteres con nombre sin los corchetes de afuera (SC2101 de ShellCheck)",
            "usá [[:digit:]]",
            "[:digit:] es el conjunto de las letras d, i, g y t.",
            "ls [:digit:] se vuelve ls [[:digit:]].",
        ),
    ),
    (
        ShellRule::Sc2102,
        RuleText::new(
            "Rango de un glob con caracteres repetidos (SC2102 de ShellCheck)",
            "usá una lista o alternativas para varios caracteres",
            "Un rango coincide con un carácter, no con una palabra.",
            "ls [10-15] coincide con un carácter, 0 o de 1 a 5.",
        ),
    ),
    (
        ShellRule::Sc2104,
        RuleText::new(
            "break o continue directo en una función (SC2104 de ShellCheck)",
            "usá return",
            "Sin un loop alrededor no hay de dónde salir.",
            "f() { break; } se vuelve f() { return; }.",
        ),
    ),
    (
        ShellRule::Sc2105,
        RuleText::new(
            "break o continue afuera de un loop (SC2105 de ShellCheck)",
            "ponelo adentro de un loop, o usá exit",
            "El shell lo rechaza o lo ignora.",
            "break se vuelve exit.",
        ),
    ),
    (
        ShellRule::Sc2106,
        RuleText::new(
            "break o continue adentro de un subshell en un loop (SC2106 de ShellCheck)",
            "sacalo del subshell, o evitá el pipeline",
            "Sólo termina el subshell, no el loop.",
            "(break) se vuelve break.",
        ),
    ),
    (
        ShellRule::Sc2111,
        RuleText::new(
            "function y () a la vez en ksh (SC2111 de ShellCheck)",
            "usá una de las dos formas",
            "ksh no permite las dos juntas.",
            "function f() { :; } se vuelve f() { :; }.",
        ),
    ),
    (
        ShellRule::Sc2112,
        RuleText::new(
            "La palabra function en sh (SC2112 de ShellCheck)",
            "sacá la palabra",
            "No es estándar.",
            "function f() { :; } se vuelve f() { :; }.",
        ),
    ),
    (
        ShellRule::Sc2113,
        RuleText::new(
            "La palabra function sin () en sh (SC2113 de ShellCheck)",
            "usá foo() en su lugar",
            "No es estándar.",
            "function f { :; } se vuelve f() { :; }.",
        ),
    ),
    (
        ShellRule::Sc2118,
        RuleText::new(
            "|& en ksh (SC2118 de ShellCheck)",
            "usá 2>&1 |",
            "En ksh, |& arranca un coproceso, no un pipe de stderr.",
            "a |& b se vuelve a 2>&1 | b.",
        ),
    ),
    (
        ShellRule::Sc2123,
        RuleText::new(
            "PATH pisado con algo que no es una ruta de búsqueda (SC2123 de ShellCheck)",
            "usá otro nombre para la variable",
            "PATH decide dónde se buscan los comandos: los siguientes dejan de encontrarse.",
            "PATH=/mi/dir se vuelve MI_DIR=/mi/dir.",
        ),
    ),
    (
        ShellRule::Sc2127,
        RuleText::new(
            "Terminador de case que este shell no soporta (SC2127 de ShellCheck)",
            "usá ;; o cambiá el shebang",
            "sh y dash sólo conocen ;;, y ;;& sólo existe en bash.",
            "a) b;& se vuelve a) b;;.",
        ),
    ),
    (
        ShellRule::Sc2147,
        RuleText::new(
            "Tilde entre comillas en PATH (SC2147 de ShellCheck)",
            "usá $HOME",
            "Varios programas no la expanden ahí.",
            "PATH=\"~/bin:$PATH\" se vuelve PATH=\"$HOME/bin:$PATH\".",
        ),
    ),
    (
        ShellRule::Sc2165,
        RuleText::new(
            "Loop adentro de otro que pisa la variable del de afuera (SC2165 de ShellCheck)",
            "usá otra variable",
            "El loop de afuera sigue con el valor del de adentro.",
            "for i in a; do for i in b; do :; done; done usa j adentro.",
        ),
    ),
    (
        ShellRule::Sc2167,
        RuleText::new(
            "Loop cuya variable pisa otro de adentro (SC2167 de ShellCheck)",
            "usá otra variable en el de adentro",
            "Este loop sigue con el valor del de adentro.",
            "for i in a; do for i in b; do :; done; done usa j adentro.",
        ),
    ),
    (
        ShellRule::Sc2187,
        RuleText::new(
            "Shebang de ash que se revisa como dash (SC2187 de ShellCheck)",
            "agregá # shellcheck shell=dash",
            "Los chequeos usan las reglas de dash para ash.",
            "#!/bin/ash necesita # shellcheck shell=dash.",
        ),
    ),
    (
        ShellRule::Sc2239,
        RuleText::new(
            "Shebang sin una ruta absoluta (SC2239 de ShellCheck)",
            "usá la ruta absoluta del intérprete",
            "El sistema busca el intérprete exactamente donde dice.",
            "#!bash se vuelve #!/bin/bash.",
        ),
    ),
    (
        ShellRule::Sc2246,
        RuleText::new(
            "Shebang que apunta a una carpeta (SC2246 de ShellCheck)",
            "apuntá al archivo del intérprete",
            "Una carpeta no puede correr el script.",
            "#!/bin/ se vuelve #!/bin/sh.",
        ),
    ),
    (
        ShellRule::Sc2258,
        RuleText::new(
            "Coma al final de un valor de for (SC2258 de ShellCheck)",
            "sacá la coma o poné comillas en el valor",
            "La coma es parte del valor, no un separador.",
            "for i in a, b se vuelve for i in a b.",
        ),
    ),
];
