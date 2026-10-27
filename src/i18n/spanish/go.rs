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
    RuleText::new(
        "Uso de algo marcado como deprecado (SA1019 de staticcheck)",
        "pasá a lo que recomienda el comentario Deprecated: de lo que se usa",
        "Lo deprecado sigue andando hasta que un día se va o deja de recibir arreglos; el comentario dice qué usar en su lugar.",
        "ioutil.ReadFile(ruta) se vuelve os.ReadFile(ruta).",
    ),
    RuleText::new(
        "Negación de una comparación entera que se lee mejor con la ley de De Morgan (QF1001 de staticcheck)",
        "invertí cada comparación y cambiá && por || (o al revés) en vez de negar el paréntesis",
        "Una negación sobre un paréntesis obliga a invertir todo de cabeza; las comparaciones ya invertidas se leen de una.",
        "!(a == b && c > 0) se vuelve a != b || c <= 0.",
    ),
    RuleText::new(
        "Método con el nombre de una interfaz conocida pero otra firma (stdmethods de govet)",
        "dale la firma de la interfaz de la biblioteca estándar o cambiale el nombre",
        "Con ese nombre, quien lo lee (y fmt, encoding/json o io) espera la interfaz conocida; con otra firma no la implementa y nadie se entera.",
        "func (t T) Seek(off int64) int64 se vuelve func (t T) Seek(off int64, whence int) (int64, error).",
    ),
    RuleText::new(
        "Variable de error del paquete que no se llama errFoo (ST1012 de staticcheck)",
        "llamala errAlgo, o ErrAlgo si se exporta",
        "El prefijo dice de entrada que es un error y no un mensaje cualquiera, y es lo que busca quien la compara con errors.Is.",
        "var notFound = errors.New(\"no está\") se vuelve var errNotFound = errors.New(\"no está\").",
    ),
    RuleText::new(
        "Tipo de una variable que el valor ya da (QF1011 de staticcheck)",
        "sacá el tipo de la declaración: se infiere del valor",
        "El tipo repetido no agrega nada y hay que cambiarlo en dos lugares si cambia.",
        "var total int64 = suma() se vuelve var total = suma().",
    ),
    RuleText::new(
        "Tipo redundante en una declaración de variable (ST1023 de staticcheck)",
        "sacá el tipo de la declaración: se infiere del valor",
        "El tipo repetido no agrega nada y hay que cambiarlo en dos lugares si cambia.",
        "var nombres []string = lista() se vuelve var nombres = lista().",
    ),
    RuleText::new(
        "switch sin valor que compara siempre lo mismo con == (QF1002 de staticcheck)",
        "pasá lo que se compara al switch y dejá sólo los valores en cada case",
        "Un switch sobre el valor dice de entrada que todos los case miran lo mismo.",
        "switch { case x == 1: ... } se vuelve switch x { case 1: ... }.",
    ),
    RuleText::new(
        "Asignación innecesaria al identificador _ (S1005 de staticcheck)",
        "sacá el _ que sobra",
        "El _ de más sólo agrega ruido: la recepción o el range ya descartan ese valor.",
        "for i, _ := range lista se vuelve for i := range lista.",
    ),
    RuleText::new(
        "if o else vacío (SA9003 de staticcheck)",
        "borrá la rama vacía o completala",
        "Una rama vacía suele ser algo que se quedó a medio escribir o una condición que sobra.",
        "if err != nil { } se vuelve if err != nil { return err }.",
    ),
    RuleText::new(
        "for infinito que empieza con un if que corta (QF1006 de staticcheck)",
        "llevá la condición negada al for y sacá el if con el break",
        "La condición en el for dice de entrada hasta cuándo se repite.",
        "for { if hecho { break } ... } se vuelve for !hecho { ... }.",
    ),
];

pub fn go(rule: GoRule) -> RuleText {
    TEXTS.get(rule.position()).copied().unwrap_or(UNKNOWN)
}
