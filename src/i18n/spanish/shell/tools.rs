use crate::i18n::catalog::RuleText;
use crate::rules::ShellRule;

// Los chequeos de Checks/Commands.hs de ShellCheck sobre programas externos.
pub const TOOLS_TEXTS: [(ShellRule, RuleText); 37] = [
    (
        ShellRule::Sc2003,
        RuleText::new(
            "expr en vez de la aritmética de la shell (SC2003 de ShellCheck)",
            "usá $((...)), ${...} o [ ]",
            "expr es un programa externo con una sintaxis incómoda; la shell ya hace la aritmética y los strings.",
            "n=$(expr $n + 1) se vuelve n=$((n + 1)).",
        ),
    ),
    (
        ShellRule::Sc2018,
        RuleText::new(
            "Rango a-z de tr que deja afuera las letras con tilde (SC2018 de ShellCheck)",
            "usá '[:lower:]'",
            "a-z sólo cubre las letras ASCII: las que tienen tilde y las de otros alfabetos quedan como estaban.",
            "tr a-z A-Z se vuelve tr '[:lower:]' '[:upper:]'.",
        ),
    ),
    (
        ShellRule::Sc2019,
        RuleText::new(
            "Rango A-Z de tr que deja afuera las letras con tilde (SC2019 de ShellCheck)",
            "usá '[:upper:]'",
            "A-Z sólo cubre las letras ASCII: las que tienen tilde y las de otros alfabetos quedan como estaban.",
            "tr A-Z a-z se vuelve tr '[:upper:]' '[:lower:]'.",
        ),
    ),
    (
        ShellRule::Sc2020,
        RuleText::new(
            "Conjunto de tr con letras repetidas, como si reemplazara palabras (SC2020 de ShellCheck)",
            "usá sed para reemplazar palabras; tr cambia carácter por carácter",
            "tr cambia cada carácter del primer conjunto por el del mismo lugar en el segundo, no reemplaza strings.",
            "tr 'hola' 'chau' se vuelve sed 's/hola/chau/g'.",
        ),
    ),
    (
        ShellRule::Sc2021,
        RuleText::new(
            "[] alrededor de una clase de tr, que también cambia los corchetes (SC2021 de ShellCheck)",
            "sacá los corchetes",
            "tr no usa corchetes para los rangos: '[a-z]' también cambia los caracteres [ y ] literales.",
            "tr '[a-z]' '[A-Z]' se vuelve tr 'a-z' 'A-Z'.",
        ),
    ),
    (
        ShellRule::Sc2022,
        RuleText::new(
            "x* en una regex de grep leído como glob (SC2022 de ShellCheck)",
            "escribí x.* si querías x seguido de cualquier cosa",
            "En una regex x* es cero o más x, no x seguido de cualquier cosa como en un glob.",
            "grep 'foo*' se vuelve grep 'foo.*'.",
        ),
    ),
    (
        ShellRule::Sc2023,
        RuleText::new(
            "time con opciones, que la palabra clave de la shell ignora (SC2023 de ShellCheck)",
            "usá command time para correr el programa time",
            "En bash y sh time es una palabra clave sin opciones; el -f o -v de man time(1) son de /usr/bin/time.",
            "time -v cmd se vuelve command time -v cmd.",
        ),
    ),
    (
        ShellRule::Sc2024,
        RuleText::new(
            "Redirección en un comando con sudo, que corre sin sudo (SC2024 de ShellCheck)",
            "usá sudo tee o sudo cat para el archivo",
            "La shell abre el archivo redirigido antes de correr sudo, con los permisos del usuario actual.",
            "sudo echo x > /etc/f se vuelve echo x | sudo tee /etc/f.",
        ),
    ),
    (
        ShellRule::Sc2028,
        RuleText::new(
            "echo con secuencias de escape que puede no expandir (SC2028 de ShellCheck)",
            "usá printf",
            "Que echo expanda \\n o \\t depende de la shell y sus opciones; printf lo hace igual en todos lados.",
            "echo \"a\\tb\" se vuelve printf 'a\\tb\\n'.",
        ),
    ),
    (
        ShellRule::Sc2029,
        RuleText::new(
            "Expansión en un comando de ssh que se hace en el cliente (SC2029 de ShellCheck)",
            "escapá el $ o usá comillas simples si tiene que expandirse en el servidor",
            "La shell local expande el comando entre comillas dobles antes de que ssh lo mande.",
            "ssh host \"echo $HOME\" muestra el HOME local; ssh host 'echo $HOME' el remoto.",
        ),
    ),
    (
        ShellRule::Sc2060,
        RuleText::new(
            "Argumento de tr sin comillas que la shell puede expandir como glob (SC2060 de ShellCheck)",
            "poné los conjuntos de tr entre comillas",
            "Sin comillas, [a-z] puede coincidir con un archivo de la carpeta y tr recibe eso en su lugar.",
            "tr [a-z] [A-Z] se vuelve tr '[:lower:]' '[:upper:]'.",
        ),
    ),
    (
        ShellRule::Sc2061,
        RuleText::new(
            "Patrón de find sin comillas que la shell expande antes (SC2061 de ShellCheck)",
            "poné entre comillas el patrón de -name, -path y similares",
            "Si un archivo de la carpeta coincide, find recibe su nombre en vez del patrón.",
            "find . -name *.log se vuelve find . -name '*.log'.",
        ),
    ),
    (
        ShellRule::Sc2062,
        RuleText::new(
            "Patrón de grep sin comillas que la shell puede expandir (SC2062 de ShellCheck)",
            "poné el patrón entre comillas",
            "Sin comillas, el patrón puede coincidir con nombres de archivo y grep recibe esos.",
            "grep foo* archivo se vuelve grep 'foo*' archivo.",
        ),
    ),
    (
        ShellRule::Sc2063,
        RuleText::new(
            "Patrón de grep que parece un glob (SC2063 de ShellCheck)",
            "escribilo como regex, o usá grep -F para texto fijo",
            "grep lee una regex: *.txt no es lo mismo que en un glob.",
            "grep '*.txt' se vuelve grep '\\.txt$'.",
        ),
    ),
    (
        ShellRule::Sc2114,
        RuleText::new(
            "rm -r sobre un directorio del sistema (SC2114 de ShellCheck)",
            "revisá la ruta: esto borra un directorio del sistema",
            "Un rm recursivo de / o /usr deja la máquina sin sistema.",
            "rm -rf /usr/ se vuelve rm -rf ./usr/.",
        ),
    ),
    (
        ShellRule::Sc2115,
        RuleText::new(
            "rm -r sobre una ruta que es un directorio del sistema si la variable está vacía (SC2115 de ShellCheck)",
            "usá \"${var:?}\" para que falle si está vacía",
            "Si la variable está vacía o sin definir, rm -rf \"$dir/\" borra la raíz.",
            "rm -rf \"$dir/\" se vuelve rm -rf \"${dir:?}/\".",
        ),
    ),
    (
        ShellRule::Sc2117,
        RuleText::new(
            "su sin comando, que abre una shell interactiva (SC2117 de ShellCheck)",
            "usá su -c 'comando' o sudo",
            "El resto del script no corre como el otro usuario: espera a que esa shell termine.",
            "su postgres; psql se vuelve su postgres -c psql.",
        ),
    ),
    (
        ShellRule::Sc2146,
        RuleText::new(
            "Acción de find que ignora los tests de antes del -o (SC2146 de ShellCheck)",
            "agrupá los tests con \\( \\)",
            "-o une menos que el -a implícito, así que la acción sólo vale para el último test.",
            "find . -name a -o -name b -delete se vuelve find . \\( -name a -o -name b \\) -delete.",
        ),
    ),
    (
        ShellRule::Sc2150,
        RuleText::new(
            "find -exec con un comando de shell en un solo argumento (SC2150 de ShellCheck)",
            "usá -exec sh -c '...' o separá el comando en argumentos",
            "-exec corre un programa, no una shell: un pipe o ; dentro de un argumento es parte del nombre.",
            "find . -exec 'grep x | wc' {} \\; se vuelve find . -exec sh -c 'grep x \"$1\" | wc' _ {} \\;.",
        ),
    ),
    (
        ShellRule::Sc2156,
        RuleText::new(
            "{} dentro del script de find -exec sh -c (SC2156 de ShellCheck)",
            "pasá el archivo como parámetro: sh -c '... \"$1\"' _ {}",
            "Un nombre de archivo con comillas o $ se vuelve código dentro del script.",
            "find . -exec sh -c 'cat {}' \\; se vuelve find . -exec sh -c 'cat \"$1\"' _ {} \\;.",
        ),
    ),
    (
        ShellRule::Sc2174,
        RuleText::new(
            "mkdir -p -m, donde el modo sólo vale para el último directorio (SC2174 de ShellCheck)",
            "creá los directorios y hacé chmod en cada uno, o ajustá umask",
            "Los directorios padre que crea mkdir -p quedan con el modo por defecto, no con el de -m.",
            "mkdir -p -m 700 a/b crea a con el modo por defecto.",
        ),
    ),
    (
        ShellRule::Sc2185,
        RuleText::new(
            "find sin ruta (SC2185 de ShellCheck)",
            "pasá . explícitamente",
            "Algunas versiones de find no tienen ruta por defecto y fallan.",
            "find -name x se vuelve find . -name x.",
        ),
    ),
    (
        ShellRule::Sc2186,
        RuleText::new(
            "tempfile, que está deprecado (SC2186 de ShellCheck)",
            "usá mktemp",
            "tempfile se sacó de Debian y otros sistemas; mktemp es la forma estándar.",
            "tmp=$(tempfile) se vuelve tmp=$(mktemp).",
        ),
    ),
    (
        ShellRule::Sc2196,
        RuleText::new(
            "egrep, que está deprecado (SC2196 de ShellCheck)",
            "usá grep -E",
            "egrep no es estándar y las versiones nuevas de grep avisan cada vez que se usa.",
            "egrep 'a|b' se vuelve grep -E 'a|b'.",
        ),
    ),
    (
        ShellRule::Sc2197,
        RuleText::new(
            "fgrep, que está deprecado (SC2197 de ShellCheck)",
            "usá grep -F",
            "fgrep no es estándar y las versiones nuevas de grep avisan cada vez que se usa.",
            "fgrep 'a.b' se vuelve grep -F 'a.b'.",
        ),
    ),
    (
        ShellRule::Sc2224,
        RuleText::new(
            "mv sin destino (SC2224 de ShellCheck)",
            "agregá el destino",
            "Con un solo argumento mv falla, y casi siempre es que se perdió un argumento.",
            "mv \"$archivo\" se vuelve mv \"$archivo\" \"$destino\".",
        ),
    ),
    (
        ShellRule::Sc2225,
        RuleText::new(
            "cp sin destino (SC2225 de ShellCheck)",
            "agregá el destino",
            "Con un solo argumento cp falla, y casi siempre es que se perdió un argumento.",
            "cp \"$archivo\" se vuelve cp \"$archivo\" \"$destino\".",
        ),
    ),
    (
        ShellRule::Sc2226,
        RuleText::new(
            "ln sin destino (SC2226 de ShellCheck)",
            "agregá el destino, o . explícitamente",
            "Con un solo argumento ln crea el enlace en la carpeta actual, y casi nunca es lo que se quería.",
            "ln -s \"$destino\" se vuelve ln -s \"$destino\" \"$enlace\".",
        ),
    ),
    (
        ShellRule::Sc2227,
        RuleText::new(
            "Redirección en medio de los argumentos de find (SC2227 de ShellCheck)",
            "pasá la redirección al final, o redirigí dentro de cada acción",
            "La redirección vale para find entero, no para la acción que tiene al lado.",
            "find . -print > a -exec x {} \\; se vuelve find . -exec x {} \\; > a.",
        ),
    ),
    (
        ShellRule::Sc2232,
        RuleText::new(
            "sudo con un builtin de la shell (SC2232 de ShellCheck)",
            "usá sudo sh -c '...'",
            "Los builtins como cd o export no son programas, así que sudo no los encuentra.",
            "sudo cd /root se vuelve sudo sh -c 'cd /root && ...'.",
        ),
    ),
    (
        ShellRule::Sc2253,
        RuleText::new(
            "chmod -r, que saca el permiso de lectura (SC2253 de ShellCheck)",
            "usá -R para recorrer, o a-r si la idea es sacar la lectura",
            "-r es el modo 'sacar lectura', no la opción recursiva.",
            "chmod -r 755 dir se vuelve chmod -R 755 dir.",
        ),
    ),
    (
        ShellRule::Sc2267,
        RuleText::new(
            "xargs -i, que está deprecado (SC2267 de ShellCheck)",
            "usá -I{}",
            "GNU xargs deprecó -i a favor de -I, que anda en todos los xargs.",
            "xargs -i cp {} dst se vuelve xargs -I{} cp {} dst.",
        ),
    ),
    (
        ShellRule::Sc2304,
        RuleText::new(
            "* sin escapar en una multiplicación de expr (SC2304 de ShellCheck)",
            "escapalo como \\* o usá $((x * y))",
            "Sin comillas, * se expande a los archivos de la carpeta.",
            "expr 2 * 3 se vuelve echo $((2 * 3)).",
        ),
    ),
    (
        ShellRule::Sc2305,
        RuleText::new(
            "Regex de expr sin comillas que la shell puede expandir (SC2305 de ShellCheck)",
            "poné la regex entre comillas",
            "Sin comillas, la regex puede coincidir con nombres de archivo y expr recibe esos.",
            "expr \"$a\" : .*x se vuelve expr \"$a\" : '.*x'.",
        ),
    ),
    (
        ShellRule::Sc2306,
        RuleText::new(
            "Carácter de glob sin escapar en un argumento de expr (SC2306 de ShellCheck)",
            "escapalo o ponelo entre comillas",
            "Sin comillas, el glob puede expandirse a nombres de archivo antes de que expr lo vea.",
            "expr 2 * 3 + 1 se vuelve expr 2 \\* 3 + 1.",
        ),
    ),
    (
        ShellRule::Sc2307,
        RuleText::new(
            "expr con menos de tres argumentos (SC2307 de ShellCheck)",
            "separá cada operando y operador en su propio argumento",
            "expr necesita los operandos y el operador como palabras separadas; '1+1' es sólo un string.",
            "expr 1+1 se vuelve expr 1 + 1.",
        ),
    ),
    (
        ShellRule::Sc2308,
        RuleText::new(
            "expr match, length, substr o index, que no son estándar (SC2308 de ShellCheck)",
            "usá expr str : regex, ${#var} o cut",
            "Estas formas son extensiones de GNU con resultados no especificados en otros lados.",
            "expr length \"$s\" se vuelve ${#s}.",
        ),
    ),
];
