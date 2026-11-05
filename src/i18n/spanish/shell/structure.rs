use crate::i18n::catalog::RuleText;
use crate::rules::ShellRule;

// Más chequeos de Analytics.hs de ShellCheck sin flujo de datos: operandos de condiciones, case, subshells, redirecciones y asignaciones.
pub const STRUCTURE_TEXTS: [(ShellRule, RuleText); 24] = [
    (
        ShellRule::Sc2001,
        RuleText::new(
            "sed sobre una variable, cuando alcanza con una expansión (SC2001 de ShellCheck)",
            "usá ${variable//buscar/reemplazo}",
            "Una expansión no arranca un proceso para un reemplazo simple.",
            "$(echo \"$a\" | sed 's/x/y/g') se vuelve ${a//x/y}.",
        ),
    ),
    (
        ShellRule::Sc2035,
        RuleText::new(
            "Glob que puede expandirse a nombres que parecen opciones (SC2035 de ShellCheck)",
            "usá ./*glob* o -- *glob*",
            "Un archivo llamado -rf lo leería el comando como una opción.",
            "rm *.txt se vuelve rm ./*.txt.",
        ),
    ),
    (
        ShellRule::Sc2037,
        RuleText::new(
            "Asignación seguida de una opción o un glob: el valor no es la salida del comando (SC2037 de ShellCheck)",
            "usá var=$(cmd) para asignar la salida",
            "A=ls -l define A sólo para el comando -l, que no existe.",
            "A=ls -l se vuelve A=$(ls -l).",
        ),
    ),
    (
        ShellRule::Sc2044,
        RuleText::new(
            "for sobre la salida de find (SC2044 de ShellCheck)",
            "usá find -exec o un while read",
            "Los nombres con espacios o saltos de línea se parten en varias palabras.",
            "for f in $(find .) se vuelve find . -exec cmd {} +.",
        ),
    ),
    (
        ShellRule::Sc2045,
        RuleText::new(
            "for sobre la salida de ls (SC2045 de ShellCheck)",
            "usá un glob",
            "Los nombres con espacios se parten, y la salida de ls está pensada para personas.",
            "for f in $(ls *.txt) se vuelve for f in *.txt.",
        ),
    ),
    (
        ShellRule::Sc2079,
        RuleText::new(
            "Número decimal en un contexto aritmético (SC2079 de ShellCheck)",
            "usá bc o awk",
            "La aritmética del shell sólo trabaja con enteros.",
            "a[1.5]=x necesita un índice entero.",
        ),
    ),
    (
        ShellRule::Sc2094,
        RuleText::new(
            "El mismo archivo se lee y se escribe en un pipeline (SC2094 de ShellCheck)",
            "escribí a un archivo temporal y movelo después",
            "La redirección vacía el archivo antes de que el comando lo lea.",
            "cat f > f deja f vacío.",
        ),
    ),
    (
        ShellRule::Sc2103,
        RuleText::new(
            "cd seguido de otro cd para volver (SC2103 de ShellCheck)",
            "usá un ( subshell ) en vez de volver a mano",
            "Si falla el primer cd, el segundo deja el script en otro directorio.",
            "cd d; x; cd .. se vuelve (cd d && x).",
        ),
    ),
    (
        ShellRule::Sc2129,
        RuleText::new(
            "Varios comandos seguidos agregan al mismo archivo (SC2129 de ShellCheck)",
            "usá { cmd1; cmd2; } >> archivo",
            "El archivo se abre una vez en lugar de una por comando.",
            "a >> f; b >> f; c >> f se vuelve { a; b; c; } >> f.",
        ),
    ),
    (
        ShellRule::Sc2141,
        RuleText::new(
            "IFS con una barra, una n o una t literales (SC2141 de ShellCheck)",
            "usá $'\\n', $'\\t' o el carácter literal",
            "Entre comillas comunes \\n es una barra y una n, y las palabras se parten en esas letras.",
            "IFS=\"\\n\" se vuelve IFS=$'\\n'.",
        ),
    ),
    (
        ShellRule::Sc2194,
        RuleText::new(
            "case sobre una palabra constante (SC2194 de ShellCheck)",
            "agregá el $ que le falta a la variable",
            "Corre siempre la misma rama.",
            "case foo in se vuelve case $foo in.",
        ),
    ),
    (
        ShellRule::Sc2195,
        RuleText::new(
            "Patrón de case que nunca coincide con la palabra (SC2195 de ShellCheck)",
            "revisá el patrón y la palabra",
            "Esa rama no corre nunca.",
            "case x$a in y*) nunca coincide.",
        ),
    ),
    (
        ShellRule::Sc2209,
        RuleText::new(
            "Nombre de un comando asignado a una variable (SC2209 de ShellCheck)",
            "usá var=$(comando) para la salida, o comillas si es el texto",
            "Parece que se buscaba la salida del comando.",
            "here=pwd se vuelve here=$(pwd).",
        ),
    ),
    (
        ShellRule::Sc2221,
        RuleText::new(
            "Patrón de case que siempre le gana a uno posterior (SC2221 de ShellCheck)",
            "poné primero el patrón más específico",
            "La rama de después no corre nunca.",
            "*.txt) antes de a.txt) deja muerto al segundo.",
        ),
    ),
    (
        ShellRule::Sc2222,
        RuleText::new(
            "Patrón de case que nunca coincide por uno anterior (SC2222 de ShellCheck)",
            "poné este patrón antes del otro, o sacalo",
            "Un patrón anterior ya cubre todo lo que éste coincide.",
            "a.txt) después de *.txt) no corre nunca.",
        ),
    ),
    (
        ShellRule::Sc2231,
        RuleText::new(
            "Expansión sin comillas en el glob de un for (SC2231 de ShellCheck)",
            "poné la expansión entre comillas, como \"$dir\"/*.txt",
            "El directorio se parte en los espacios antes de aplicar el glob.",
            "for f in $d/*.txt se vuelve for f in \"$d\"/*.txt.",
        ),
    ),
    (
        ShellRule::Sc2233,
        RuleText::new(
            "Paréntesis de más alrededor de una condición (SC2233 de ShellCheck)",
            "sacá los ( )",
            "Arrancan un subshell sólo para evaluar una prueba.",
            "if ( [ a ] ); then se vuelve if [ a ]; then.",
        ),
    ),
    (
        ShellRule::Sc2234,
        RuleText::new(
            "Paréntesis de más alrededor de una prueba (SC2234 de ShellCheck)",
            "sacá los ( )",
            "Arrancan un subshell sólo para evaluar una prueba.",
            "( [ a ] ) se vuelve [ a ].",
        ),
    ),
    (
        ShellRule::Sc2235,
        RuleText::new(
            "Subshell que sólo agrupa pruebas (SC2235 de ShellCheck)",
            "usá { ..; } en vez de (..)",
            "Las llaves agrupan igual sin arrancar un subshell.",
            "( [ a ] || [ b ] ) se vuelve { [ a ] || [ b ]; }.",
        ),
    ),
    (
        ShellRule::Sc2269,
        RuleText::new(
            "Variable asignada a sí misma (SC2269 de ShellCheck)",
            "sacá la asignación",
            "La asignación no hace nada.",
            "x=$x se va.",
        ),
    ),
    (
        ShellRule::Sc2295,
        RuleText::new(
            "Expansión sin comillas en un patrón de ${..} (SC2295 de ShellCheck)",
            "poné la expansión entre comillas aparte",
            "Si no, su valor se lee como patrón y no como texto.",
            "${a%$b} se vuelve ${a%\"$b\"}.",
        ),
    ),
    (
        ShellRule::Sc2321,
        RuleText::new(
            "$(( )) adentro del índice de un arreglo (SC2321 de ShellCheck)",
            "sacá el $(( y el ))",
            "Los índices de un arreglo común ya son un contexto aritmético.",
            "a[$((i+1))]=x se vuelve a[i+1]=x.",
        ),
    ),
    (
        ShellRule::Sc2327,
        RuleText::new(
            "Sustitución de comando cuya salida se redirige a otro lado (SC2327 de ShellCheck)",
            "sacá la redirección, o usá tee",
            "La sustitución queda siempre vacía.",
            "x=$(cmd > f) deja x vacía.",
        ),
    ),
    (
        ShellRule::Sc2328,
        RuleText::new(
            "La redirección le saca la salida a una sustitución de comando (SC2328 de ShellCheck)",
            "usá tee para duplicar la salida",
            "A la sustitución no le llega nada.",
            "x=$(cmd > f) se vuelve x=$(cmd | tee f).",
        ),
    ),
];
