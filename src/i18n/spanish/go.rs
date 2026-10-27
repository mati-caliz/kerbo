use crate::i18n::catalog::RuleText;
use crate::rules::GoRule;

const UNKNOWN: RuleText = RuleText::new(
    "Regla de golangci-lint",
    "seguí la indicación de la regla",
    "golangci-lint la marca por defecto.",
    "Ver la documentación de golangci-lint.",
);
const TEXTS: [RuleText; GoRule::ALL.len()] = [
    RuleText::new(
        "Asignación que nadie lee antes de que se pise o termine la función",
        "borrá la asignación o usá el valor antes de volver a asignar",
        "El valor se calcula y se tira: o sobra el cálculo o falta usarlo, y lo segundo suele ser un error que se perdió, como un err que nadie revisa.",
        "x := cargar(); x = otro() se vuelve x := otro().",
    ),
    RuleText::new(
        "Error que devuelve una llamada y nadie revisa (errcheck)",
        "revisá el error, devolvelo o, si de verdad no importa, asignalo a _ con un comentario del motivo",
        "Un error ignorado deja seguir como si todo hubiera salido bien: un archivo que no se cerró, un dato que no se guardó, una conexión que se cayó.",
        "f.Close() se vuelve if err := f.Close(); err != nil { return err }.",
    ),
    RuleText::new(
        "Mensaje de error con mayúscula al principio o puntuación al final (ST1005 de staticcheck)",
        "empezá el mensaje en minúscula y sacale el punto, los dos puntos, el signo o el salto de línea del final",
        "Los errores se encadenan: fmt.Errorf(\"guardar: %w\", err) arma una sola oración, y una mayúscula o un punto en el medio la cortan.",
        "errors.New(\"No se encontró el pedido.\") se vuelve errors.New(\"no se encontró el pedido\").",
    ),
    RuleText::new(
        "Cadena de if y else if que compara siempre lo mismo con == (QF1003 de staticcheck)",
        "pasala a un switch sobre lo que se compara",
        "Un switch dice de entrada que todas las ramas miran el mismo valor; la cadena de if obliga a leer cada condición para darse cuenta.",
        "if x == 1 { a() } else if x == 2 { b() } se vuelve switch x { case 1: a(); case 2: b() }.",
    ),
    RuleText::new(
        "strings.Replace, strings.SplitN o sus pares de bytes con -1 como límite (QF1004 de staticcheck)",
        "usá strings.ReplaceAll, strings.Split o strings.SplitAfter sin el -1",
        "El -1 sólo quiere decir \"sin límite\": la función sin límite lo dice con el nombre y no deja un número mágico.",
        "strings.Replace(s, \"a\", \"b\", -1) se vuelve strings.ReplaceAll(s, \"a\", \"b\").",
    ),
    RuleText::new(
        "Campo embebido que sobra en una cadena de selectores (QF1008 de staticcheck)",
        "sacá el campo embebido: el selector llega al mismo campo o método sin nombrarlo",
        "Nombrar el campo embebido alarga la expresión sin decir nada nuevo y ata el código a cómo está armado el tipo por dentro.",
        "pedido.Base.ID se vuelve pedido.ID.",
    ),
    RuleText::new(
        "Escritura de un fmt.Sprintf armado aparte en un io.Writer (QF1012 de staticcheck)",
        "usá fmt.Fprintf (o Fprint, Fprintln) sobre el destino",
        "Fprintf escribe directo en el destino; con Sprintf se arma un texto intermedio que se tira enseguida.",
        "w.Write([]byte(fmt.Sprintf(\"%d\", n))) se vuelve fmt.Fprintf(w, \"%d\", n).",
    ),
];

pub fn go(rule: GoRule) -> RuleText {
    TEXTS.get(rule.position()).copied().unwrap_or(UNKNOWN)
}
