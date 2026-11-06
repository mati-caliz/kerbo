use crate::i18n::catalog::RuleText;
use crate::rules::ShellRule;

// Chequeos de Analytics.hs de ShellCheck sobre lo que se compara y cómo: operadores de condiciones, && y || encadenados, aritmética y comandos que parecen comparaciones.
pub const COMPARISONS_TEXTS: [(ShellRule, RuleText); 43] = [
    (
        ShellRule::Sc2017,
        RuleText::new(
            "División antes de multiplicar, que pierde precisión (SC2017 de ShellCheck)",
            "multiplicá primero: a*c/b",
            "La división entera trunca antes de que llegue la multiplicación.",
            "$((a/b*c)) se vuelve $((a*c/b)).",
        ),
    ),
    (
        ShellRule::Sc2049,
        RuleText::new(
            "=~ con algo que parece un glob (SC2049 de ShellCheck)",
            "usá = para globs, o escribí una expresión regular de verdad",
            "=~ lee el lado derecho como expresión regular, donde * no significa cualquier texto.",
            "[[ $a =~ *.txt ]] se vuelve [[ $a = *.txt ]].",
        ),
    ),
    (
        ShellRule::Sc2053,
        RuleText::new(
            "Lado derecho sin comillas en una comparación de [[ ]] (SC2053 de ShellCheck)",
            "poné comillas en el lado derecho",
            "Sin comillas, el valor de la variable se lee como un glob y no como texto.",
            "[[ $a = $b ]] se vuelve [[ $a = \"$b\" ]].",
        ),
    ),
    (
        ShellRule::Sc2054,
        RuleText::new(
            "Comas entre los elementos de un arreglo (SC2054 de ShellCheck)",
            "separá los elementos con espacios",
            "La coma queda adentro del elemento, porque los arreglos se parten por los espacios.",
            "a=(1, 2) se vuelve a=(1 2).",
        ),
    ),
    (
        ShellRule::Sc2055,
        RuleText::new(
            "Dos valores distintos comparados con != unidos con o (SC2055 de ShellCheck)",
            "usá -a o && en vez de -o o ||",
            "Un valor siempre es distinto de uno de dos valores distintos: la condición siempre da verdadero.",
            "[ $a != x -o $a != y ] se vuelve [ $a != x -a $a != y ].",
        ),
    ),
    (
        ShellRule::Sc2056,
        RuleText::new(
            "Dos != del mismo valor unidos con || en aritmética (SC2056 de ShellCheck)",
            "usá && en vez de ||",
            "Un valor siempre es distinto de uno de dos valores distintos: la condición siempre da verdadero.",
            "((a != 1 || a != 2)) se vuelve ((a != 1 && a != 2)).",
        ),
    ),
    (
        ShellRule::Sc2057,
        RuleText::new(
            "Operador binario desconocido en una condición (SC2057 de ShellCheck)",
            "usá un operador que exista",
            "El shell rechaza la condición al ejecutarla.",
            "[ $a -foo $b ] se vuelve [ $a -eq $b ].",
        ),
    ),
    (
        ShellRule::Sc2058,
        RuleText::new(
            "Operador unario desconocido en una condición (SC2058 de ShellCheck)",
            "usá un operador que exista",
            "El shell rechaza la condición al ejecutarla.",
            "[ -foo $a ] se vuelve [ -f $a ].",
        ),
    ),
    (
        ShellRule::Sc2065,
        RuleText::new(
            "> o < después de test es una redirección a un archivo (SC2065 de ShellCheck)",
            "usá -gt o -lt, o escapá el operador",
            "El shell crea o lee un archivo con ese nombre y test nunca ve la comparación.",
            "test 1 > 2 se vuelve test 1 -gt 2.",
        ),
    ),
    (
        ShellRule::Sc2071,
        RuleText::new(
            "Operador de texto comparando números (SC2071 de ShellCheck)",
            "usá -lt, -gt, -le o -ge",
            "< y > comparan texto: 10 queda antes que 9.",
            "[[ $a > 3 ]] se vuelve [[ $a -gt 3 ]].",
        ),
    ),
    (
        ShellRule::Sc2072,
        RuleText::new(
            "Número con decimales en una comparación (SC2072 de ShellCheck)",
            "usá enteros, o compará con bc o awk",
            "El shell sólo compara enteros.",
            "[ $a -lt 1.5 ] se vuelve una comparación en awk.",
        ),
    ),
    (
        ShellRule::Sc2073,
        RuleText::new(
            "< o > sin escapar en [ ] (SC2073 de ShellCheck)",
            "escapá el operador o usá [[ ]]",
            "Sin escapar redirige a un archivo en vez de comparar.",
            "[ a < b ] se vuelve [ a \\< b ].",
        ),
    ),
    (
        ShellRule::Sc2074,
        RuleText::new(
            "=~ en [ ] (SC2074 de ShellCheck)",
            "usá [[ ]]",
            "=~ sólo existe en [[ ]].",
            "[ $a =~ b ] se vuelve [[ $a =~ b ]].",
        ),
    ),
    (
        ShellRule::Sc2080,
        RuleText::new(
            "Número con un 0 adelante que no es octal válido (SC2080 de ShellCheck)",
            "sacá el 0, o escribí 10#08",
            "Un 0 adelante significa octal, donde 8 y 9 no existen.",
            "((a = 08)) se vuelve ((a = 8)).",
        ),
    ),
    (
        ShellRule::Sc2081,
        RuleText::new(
            "Glob en una comparación de [ ] (SC2081 de ShellCheck)",
            "usá [[ ]] o un case",
            "[ ] compara texto: el glob no se usa como patrón.",
            "[ $a = *.txt ] se vuelve [[ $a = *.txt ]].",
        ),
    ),
    (
        ShellRule::Sc2099,
        RuleText::new(
            "Aritmética escrita como un comando después de una asignación (SC2099 de ShellCheck)",
            "usá $((..))",
            "El shell ejecuta + como un comando, con una variable temporal.",
            "i=$i + 1 se vuelve i=$((i + 1)).",
        ),
    ),
    (
        ShellRule::Sc2100,
        RuleText::new(
            "Aritmética asignada como texto (SC2100 de ShellCheck)",
            "usá $((..))",
            "La variable recibe el texto, sin calcular nada.",
            "i=i+1 se vuelve i=$((i + 1)).",
        ),
    ),
    (
        ShellRule::Sc2107,
        RuleText::new(
            "&& adentro de [ ] (SC2107 de ShellCheck)",
            "usá [ a ] && [ b ]",
            "[ ] no conoce && y el shell corta el comando ahí.",
            "[ a && b ] se vuelve [ a ] && [ b ].",
        ),
    ),
    (
        ShellRule::Sc2108,
        RuleText::new(
            "-a adentro de [[ ]] (SC2108 de ShellCheck)",
            "usá &&",
            "[[ ]] no acepta -a como y lógico.",
            "[[ a -a b ]] se vuelve [[ a && b ]].",
        ),
    ),
    (
        ShellRule::Sc2109,
        RuleText::new(
            "|| adentro de [ ] (SC2109 de ShellCheck)",
            "usá [ a ] || [ b ]",
            "[ ] no conoce || y el shell corta el comando ahí.",
            "[ a || b ] se vuelve [ a ] || [ b ].",
        ),
    ),
    (
        ShellRule::Sc2110,
        RuleText::new(
            "-o adentro de [[ ]] (SC2110 de ShellCheck)",
            "usá ||",
            "[[ ]] no acepta -o como o lógico.",
            "[[ a -o b ]] se vuelve [[ a || b ]].",
        ),
    ),
    (
        ShellRule::Sc2122,
        RuleText::new(
            "<= o >= en una comparación de texto (SC2122 de ShellCheck)",
            "usá ! a > b, o -le y -ge para números",
            "Esos operadores no existen para texto.",
            "[[ a <= b ]] se vuelve [[ ! a > b ]].",
        ),
    ),
    (
        ShellRule::Sc2170,
        RuleText::new(
            "Texto donde [ ] espera un número (SC2170 de ShellCheck)",
            "usá = para comparar texto, o $var para una variable",
            "-eq y compañía fallan si el operando no es un número.",
            "[ $a -eq foo ] se vuelve [ $a = foo ].",
        ),
    ),
    (
        ShellRule::Sc2171,
        RuleText::new(
            "] o ]] de cierre sin su apertura (SC2171 de ShellCheck)",
            "agregá la apertura, o poné comillas si es literal",
            "El cierre pasa como un argumento más: el comando no es una condición.",
            "echo a ] se vuelve [ a ].",
        ),
    ),
    (
        ShellRule::Sc2204,
        RuleText::new(
            "( ) con un operador de condición (SC2204 de ShellCheck)",
            "usá [ ] para una condición",
            "( ) abre un subshell y ejecuta -e como un comando.",
            "( -e file ) se vuelve [ -e file ].",
        ),
    ),
    (
        ShellRule::Sc2205,
        RuleText::new(
            "( ) con un operador de comparación (SC2205 de ShellCheck)",
            "usá [ ] para una condición",
            "( ) abre un subshell y ejecuta la primera palabra como un comando.",
            "( a = b ) se vuelve [ a = b ].",
        ),
    ),
    (
        ShellRule::Sc2212,
        RuleText::new(
            "[ ] vacío (SC2212 de ShellCheck)",
            "usá false",
            "Una condición vacía es una forma oscura de escribir false.",
            "[ ] se vuelve false.",
        ),
    ),
    (
        ShellRule::Sc2252,
        RuleText::new(
            "Dos condiciones != con valores distintos unidas con || (SC2252 de ShellCheck)",
            "usá && en vez de ||",
            "Un valor siempre es distinto de uno de dos valores distintos: el resultado siempre es verdadero.",
            "[ $a != x ] || [ $a != y ] se vuelve [ $a != x ] && [ $a != y ].",
        ),
    ),
    (
        ShellRule::Sc2254,
        RuleText::new(
            "Expansión sin comillas en un patrón de case (SC2254 de ShellCheck)",
            "poné comillas en la expansión",
            "Sin comillas, el valor se lee como un patrón y no como texto.",
            "case $a in $b) se vuelve case $a in \"$b\").",
        ),
    ),
    (
        ShellRule::Sc2257,
        RuleText::new(
            "Aritmética que modifica una variable adentro de una redirección (SC2257 de ShellCheck)",
            "hacé la modificación en una sentencia aparte",
            "El shell puede descartar ese cambio.",
            "cmd > $((i++)) se vuelve cmd > \"$i\"; ((i++)).",
        ),
    ),
    (
        ShellRule::Sc2265,
        RuleText::new(
            "& después de una condición (SC2265 de ShellCheck)",
            "usá &&",
            "Un solo & manda la condición al fondo y siempre sigue.",
            "[ a ] & [ b ] se vuelve [ a ] && [ b ].",
        ),
    ),
    (
        ShellRule::Sc2266,
        RuleText::new(
            "| al lado de una condición (SC2266 de ShellCheck)",
            "usá ||",
            "Un solo | conecta la salida en vez de funcionar como o lógico.",
            "[ a ] | [ b ] se vuelve [ a ] || [ b ].",
        ),
    ),
    (
        ShellRule::Sc2283,
        RuleText::new(
            "= como segunda palabra de un comando (SC2283 de ShellCheck)",
            "sacá los espacios para asignar, o usá [ ] para comparar",
            "El shell ejecuta la primera palabra como un comando, con = de argumento.",
            "a = 1 se vuelve a=1.",
        ),
    ),
    (
        ShellRule::Sc2284,
        RuleText::new(
            "== como segunda palabra de un comando (SC2284 de ShellCheck)",
            "usá [ a = b ] para comparar",
            "El shell ejecuta la primera palabra como un comando, con == de argumento.",
            "a == 1 se vuelve [ \"$a\" = 1 ].",
        ),
    ),
    (
        ShellRule::Sc2285,
        RuleText::new(
            "+= como segunda palabra de un comando (SC2285 de ShellCheck)",
            "sacá los espacios alrededor de +=",
            "El shell ejecuta la primera palabra como un comando, con += de argumento.",
            "a += 1 se vuelve a+=1.",
        ),
    ),
    (
        ShellRule::Sc2309,
        RuleText::new(
            "-eq y compañía leen texto como aritmética en [[ ]] (SC2309 de ShellCheck)",
            "usá = para comparar texto, o expandilo explícito con $var",
            "Una palabra que no es un número se evalúa como una variable o una cuenta.",
            "[[ $a -eq foo ]] se vuelve [[ $a = foo ]].",
        ),
    ),
    (
        ShellRule::Sc2322,
        RuleText::new(
            "Doble paréntesis adentro de una cuenta (SC2322 de ShellCheck)",
            "dejá una sola capa de paréntesis",
            "((x)) es lo mismo que (x).",
            "$(( ((a)) + 1 )) se vuelve $(( (a) + 1 )).",
        ),
    ),
    (
        ShellRule::Sc2323,
        RuleText::new(
            "Paréntesis alrededor de toda una cuenta (SC2323 de ShellCheck)",
            "sacá los paréntesis de más",
            "$(( (x) )) es lo mismo que $(( x )).",
            "$(( (a + 1) )) se vuelve $(( a + 1 )).",
        ),
    ),
    (
        ShellRule::Sc2324,
        RuleText::new(
            "+= con un número sobre una variable que no es entera (SC2324 de ShellCheck)",
            "usá (( var += 1 )) o typeset -i var, o poné comillas en el número",
            "+= agrega texto: 1 += 1 da 11.",
            "a+=1 se vuelve (( a += 1 )).",
        ),
    ),
    (
        ShellRule::Sc2330,
        RuleText::new(
            "Glob en [[ ]] con BusyBox (SC2330 de ShellCheck)",
            "usá un case",
            "[[ ]] de BusyBox no compara contra patrones.",
            "[[ $a = *.txt ]] se vuelve case \"$a\" in *.txt).",
        ),
    ),
    (
        ShellRule::Sc2331,
        RuleText::new(
            "-a para ver si un archivo existe (SC2331 de ShellCheck)",
            "usá -e",
            "-a es la forma vieja de -e y se confunde con el y lógico.",
            "[ -a file ] se vuelve [ -e file ].",
        ),
    ),
    (
        ShellRule::Sc2333,
        RuleText::new(
            "Dos = del mismo valor unidos con y (SC2333 de ShellCheck)",
            "usá -o o || en su lugar",
            "Un valor no puede ser igual a dos valores distintos: la condición siempre da falso.",
            "[ $a = x ] && [ $a = y ] se vuelve [ $a = x ] || [ $a = y ].",
        ),
    ),
    (
        ShellRule::Sc2334,
        RuleText::new(
            "Dos == del mismo valor unidos con && en aritmética (SC2334 de ShellCheck)",
            "usá || en vez de &&",
            "Un valor no puede ser igual a dos números distintos: la condición siempre da falso.",
            "((a == 1 && a == 2)) se vuelve ((a == 1 || a == 2)).",
        ),
    ),
];
