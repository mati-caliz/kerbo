use crate::i18n::catalog::RuleText;
use crate::rules::TaintRule;

const UNKNOWN: RuleText = RuleText::new(
    "Dato del usuario que llega sin limpiar a un lugar peligroso",
    "limpialo o pasalo como parámetro antes de usarlo",
    "Algo que mandó el usuario llega sin limpiar a una consulta, un comando, una ruta o la respuesta.",
    "Ver la explicación de la regla.",
);
const TEXTS: [RuleText; TaintRule::ALL.len()] = [
    RuleText::new(
        "Consulta SQL armada con datos del usuario",
        "pasá el dato como parámetro (? con setString, o los argumentos de JdbcTemplate)",
        "Algo que mandó el usuario (un parámetro, un header, una cookie, un @RequestParam) llega sin limpiar al texto de una consulta: con una comilla puede cambiarla y leer o borrar datos ajenos. Como las reglas de inyección de FindSecBugs y Semgrep.",
        "statement.executeQuery(\"SELECT * FROM usuarios WHERE nombre = '\" + nombre + \"'\") se vuelve prepareStatement(\"SELECT * FROM usuarios WHERE nombre = ?\") con setString(1, nombre).",
    ),
    RuleText::new(
        "Comando del sistema armado con datos del usuario",
        "no pases datos del usuario al comando; si hace falta, validalos contra una lista cerrada",
        "Algo que mandó el usuario llega sin limpiar a Runtime.exec o a un ProcessBuilder: con un ; o un && puede correr cualquier comando en el servidor. Como COMMAND_INJECTION de FindSecBugs.",
        "Runtime.getRuntime().exec(\"ping \" + host) se vuelve un ProcessBuilder con host validado contra los valores permitidos.",
    ),
    RuleText::new(
        "Ruta de archivo armada con datos del usuario",
        "resolvé la ruta contra un directorio fijo y verificá que no se salga de él",
        "Algo que mandó el usuario llega sin limpiar a un File, un FileInputStream o Paths.get: con ../ puede leer o pisar cualquier archivo que vea el proceso. Como PATH_TRAVERSAL_IN de FindSecBugs.",
        "new File(base, nombre) se vuelve base.resolve(nombre).normalize() con un chequeo de que empiece con base.",
    ),
    RuleText::new(
        "Respuesta HTML armada con datos del usuario",
        "escapalo antes de escribirlo (HtmlUtils.htmlEscape, Encode.forHtml o el escape de tu plantilla)",
        "Algo que mandó el usuario llega sin escapar a lo que se escribe en la respuesta: con un <script> puede correr código en el navegador de otro usuario y robarle la sesión. Como XSS_SERVLET de FindSecBugs.",
        "response.getWriter().print(\"Hola \" + nombre) se vuelve response.getWriter().print(\"Hola \" + HtmlUtils.htmlEscape(nombre)).",
    ),
];

pub fn taint(rule: TaintRule) -> RuleText {
    TEXTS.get(rule.position()).copied().unwrap_or(UNKNOWN)
}
