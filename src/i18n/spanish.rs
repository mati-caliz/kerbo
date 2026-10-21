use super::catalog::{
    BlockUnit, Catalog, Cheat, Explanation, Message, Notice, Problem, Summary, Text, counted, with_subject,
};
use crate::lint::{ImportRedundancy, LintKind};
use crate::rules::RuleId;

mod deploy;
mod dockerfile;
mod explanations;
mod fixes;

const ANONYMOUS_FUNCTION: &str = "(anónima)";
mod python;
mod security;

#[derive(Debug)]
pub struct Spanish;

impl Catalog for Spanish {
    fn text(&self, text: Text) -> &'static str {
        match text {
            Text::WhyLabel => "Por qué",
            Text::FixLabel => "Cómo se arregla",
            Text::ExampleLabel => "Ejemplo",
            Text::McpInstructions => {
                "Usá kerbo_check después de editar código: con diff (por ejemplo HEAD) mira sólo lo cambiado y detecta trampas como supresiones nuevas o tests salteados. kerbo_fix aplica de una vez los arreglos mecánicos (con dry_run sólo devuelve el parche). kerbo_explain dice cómo arreglar cada regla."
            }
            Text::McpCheckDescription => {
                "Analiza archivos o carpetas y devuelve los problemas priorizados, con id estable y pista de arreglo."
            }
            Text::McpPathsDescription => "Archivos o carpetas; por defecto la carpeta del proyecto",
            Text::McpDiffDescription => "Referencia de git: sólo lo cambiado desde ahí, con antitrampa",
            Text::McpExplainDescription => {
                "Explica una regla de kerbo: por qué existe, cómo se arregla y un ejemplo."
            }
            Text::McpFixDescription => {
                "Aplica de una vez los arreglos mecánicos que no se pisan entre sí, vuelve a analizar lo arreglado y devuelve el parche."
            }
            Text::McpRulesDescription => "Sólo los arreglos de estas reglas; por defecto, todos",
            Text::McpDryRunDescription => "Devuelve el parche sin escribir los archivos",
            Text::AboutFix => {
                "Aplica de una vez todos los arreglos mecánicos que no se pisan y vuelve a analizar lo arreglado"
            }
            Text::HelpFixRule => "Sólo los arreglos de esta regla (se puede repetir)",
            Text::HelpDryRun => "Muestra el parche sin escribir los archivos",
            Text::FixAllTitle => "arreglar todo lo mecánico del archivo",
            Text::AboutKerbo => "Analizador de calidad multi-lenguaje",
            Text::AboutCheck => "Analiza los archivos y lista los problemas",
            Text::AboutBaseline => {
                "Guarda los problemas actuales como línea de base: después sólo se reportan los nuevos"
            }
            Text::AboutExplain => "Explica una regla: por qué existe y cómo se arregla",
            Text::AboutHook => {
                "Hook para agentes: revisa el archivo recién editado (lee el evento de Claude Code por stdin)"
            }
            Text::AboutMcp => {
                "Servidor MCP por stdio: herramientas kerbo_check, kerbo_fix y kerbo_explain para agentes"
            }
            Text::AboutLsp => {
                "Servidor LSP por stdio: los problemas en el editor mientras escribís (IntelliJ, VS Code, Neovim)"
            }
            Text::HelpRule => "La regla, por ejemplo cognitive-complexity",
            Text::HelpConfig => "Usa esta configuración en vez de buscar kerbo.toml",
            Text::HelpPaths => "Archivos o carpetas a analizar",
            Text::HelpLanguages => "Analizar sólo estas pilas (se puede repetir)",
            Text::HelpMax => {
                "Cambia el máximo de una regla en todos los lenguajes, también en los tests (se puede repetir)"
            }
            Text::HelpFormat => "Formato de salida",
            Text::HelpDiff => "Sólo los problemas en líneas cambiadas desde esta referencia de git",
            Text::HelpNoBaseline => "Reporta también lo que está en la línea de base",
            Text::HelpLang => "Idioma de los mensajes; si no se indica, el de kerbo.toml o el del sistema",
            Text::ValueFile => "ARCHIVO",
            Text::ValueRuleLimit => "REGLA=VALOR",
            Text::ValueReference => "REFERENCIA",
            Text::AboutOsvDownload => {
                "Baja la base de OSV de cada ecosistema para usar `kerbo check --osv=offline` sin red"
            }
            Text::HelpOsv => {
                "Busca vulnerabilidades de las dependencias en OSV: online consulta api.osv.dev, offline usa la base bajada"
            }
            Text::HelpEcosystems => "Ecosistemas a bajar; por defecto, todos los que kerbo lee",
        }
    }

    fn rule_description(&self, rule: RuleId) -> &'static str {
        match rule {
            RuleId::FileLength => "El archivo supera el máximo de líneas",
            RuleId::FunctionLength => "La función supera el máximo de líneas",
            RuleId::CyclomaticComplexity => "La función tiene demasiados caminos posibles",
            RuleId::CognitiveComplexity => "La función es difícil de leer: demasiadas ramas anidadas",
            RuleId::ParameterCount => "La función recibe demasiados parámetros",
            RuleId::SuppressionAdded => "El cambio silencia una herramienta en vez de arreglar la causa",
            RuleId::TestDisabled => "El cambio saltea o enfoca un test, y deja otros sin correr",
            RuleId::TestRemoved => "El cambio borra tests",
            RuleId::TypeEscape => "El cambio escapa del sistema de tipos",
            RuleId::NestingDepth => "Demasiados bloques anidados",
            RuleId::MagicNumber => "Un número sin nombre en medio de la lógica",
            RuleId::ShortName => "Un nombre demasiado corto o ambiguo para lo que abarca",
            RuleId::DuplicateCode => "Un bloque de código copiado y pegado en otro lugar",
            RuleId::CommentedCode => "Código comentado en vez de borrado",
            RuleId::UnusedImport => "Un import que no se usa",
            RuleId::RedundantImport => "Un import que sobra: repetido, de java.lang o del mismo paquete",
            RuleId::StarImport => "Un import con comodín que trae el paquete entero",
            RuleId::RedundantModifier => "Un modificador que ya está implícito",
            RuleId::FinalParameter => "Un parámetro que no es final",
            RuleId::FinalClass => "Una clase que nadie puede heredar y no es final",
            RuleId::Migration(rule) => deploy::migration(rule).description,
            RuleId::Dockerfile(rule) => dockerfile::dockerfile(rule).description,
            RuleId::Compose(rule) => deploy::compose(rule).description,
            RuleId::CommittedSecret => security::COMMITTED_SECRET_DESCRIPTION,
            RuleId::VulnerableDependency => security::VULNERABLE_DEPENDENCY_DESCRIPTION,
            RuleId::Python(rule) => python::python(rule).description,
        }
    }

    fn explanation(&self, rule: RuleId) -> Explanation {
        explanations::explanation(rule)
    }

    fn message(&self, message: &Message) -> String {
        match message {
            Message::FileTooLong { lines, limit } => {
                format!("El archivo tiene {lines} líneas (máximo {limit})")
            }
            Message::FunctionTooLong {
                function,
                lines,
                limit,
            } => {
                let function = function.shown_as(ANONYMOUS_FUNCTION);
                format!("La función `{function}` tiene {lines} líneas (máximo {limit})")
            }
            Message::CyclomaticComplexity {
                function,
                value,
                limit,
            } => {
                let function = function.shown_as(ANONYMOUS_FUNCTION);
                format!("La función `{function}` tiene complejidad ciclomática {value} (máximo {limit})")
            }
            Message::CognitiveComplexity {
                function,
                value,
                limit,
            } => {
                let function = function.shown_as(ANONYMOUS_FUNCTION);
                format!("La función `{function}` tiene complejidad cognitiva {value} (máximo {limit})")
            }
            Message::TooManyParameters {
                function,
                count,
                limit,
            } => {
                let function = function.shown_as(ANONYMOUS_FUNCTION);
                format!("La función `{function}` recibe {count} parámetros (máximo {limit})")
            }
            Message::NestedTooDeep { depth, limit } => {
                format!("Este bloque está {depth} niveles adentro (máximo {limit})")
            }
            Message::UnnamedNumber { number } => format!("El número `{number}` no tiene nombre"),
            Message::ShortName { name } => format!("`{name}` es un nombre demasiado corto o ambiguo"),
            Message::RepeatedFunction { original_line } => {
                format!("Esta función repite la de la línea {original_line}")
            }
            Message::RepeatedBranch { original_line } => {
                format!("Esta rama repite la de la línea {original_line}")
            }
            Message::DuplicatedBlock {
                size,
                unit,
                copies,
                limit,
            } => format!(
                "Este bloque de {size} {} está repetido en {copies} (máximo {limit})",
                block_unit(*unit)
            ),
            Message::Lint { kind, subject } => self.lint(*kind, subject),
            Message::Cheat(cheat) => self.cheat(cheat),
            Message::CommittedSecret(leak) => security::secret(leak),
            Message::VulnerableDependency(advisory) => security::advisory(advisory),
            Message::Rule(rule) => self.rule_description(*rule).to_owned(),
            Message::Violation { rule, subject } => with_subject(self.rule_description(*rule), subject),
        }
    }

    fn lint(&self, kind: LintKind, subject: &str) -> String {
        match kind {
            LintKind::UnusedImport => format!("El import `{subject}` no se usa"),
            LintKind::RedundantImport(ImportRedundancy::JavaLang) => {
                format!("`{subject}` es de java.lang, que se importa solo")
            }
            LintKind::RedundantImport(ImportRedundancy::SamePackage) => {
                format!("`{subject}` es del mismo paquete: no hace falta importarlo")
            }
            LintKind::RedundantImport(ImportRedundancy::Duplicate { first_line }) => {
                format!("`{subject}` ya se importa en la línea {first_line}")
            }
            LintKind::StarImport => format!("`{subject}` importa el paquete entero"),
            LintKind::RedundantModifier => format!("El modificador `{subject}` sobra: ya está implícito"),
            LintKind::FinalParameter => format!("El parámetro `{subject}` debería ser final"),
            LintKind::FinalClass => {
                format!("La clase `{subject}` debería ser final: no se puede heredar desde afuera")
            }
            LintKind::Python(rule) => with_subject(python::python(rule).description, subject),
        }
    }

    fn cheat(&self, cheat: &Cheat) -> String {
        match cheat {
            Cheat::TestsRemoved {
                removed,
                before,
                after,
                net,
            } => format!(
                "{} de este archivo ({before} antes, {after} ahora); en todo el cambio hay {} menos",
                counted(*removed, "test borrado", "tests borrados"),
                counted(*net, "test", "tests")
            ),
            Cheat::EmptyTest { name } => format!("El test `{name}` quedó vacío: pasa sin probar nada"),
            Cheat::TestSkipped { marker } => {
                format!("`{marker}` deja tests sin correr: arreglá el test en vez de saltearlo")
            }
            Cheat::TypeEscaped { marker } => {
                format!("`{marker}` escapa del sistema de tipos: tipá el valor de verdad")
            }
            Cheat::SuppressionAdded { marker } => {
                format!(
                    "Se agregó `{marker}`: arreglá la causa, o dejá la excepción en kerbo.toml con su motivo"
                )
            }
            Cheat::ThresholdLoosened { old, new } => {
                format!(
                    "Se aflojó un umbral: `{old}` pasó a `{new}`. Arreglá el código en vez de bajar la vara"
                )
            }
        }
    }

    fn problem(&self, problem: &Problem) -> String {
        match problem {
            Problem::CurrentDirectory => "no se pudo leer la carpeta actual".to_owned(),
            Problem::WriteReport(detail) => format!("no se pudo escribir el reporte: {detail}"),
            Problem::Unreadable { path, detail } => format!("{path}: {detail}"),
            Problem::Unparsable { path } => format!("{path}: tree-sitter no devolvió un árbol"),
            Problem::MissingReason { path } => {
                format!("{path}: cada ajuste necesita un `reason` que diga por qué se afloja")
            }
            Problem::MaxWithoutRule { path } => {
                format!("{path}: una excepción con `max` tiene que nombrar la `rule`")
            }
            Problem::InvalidPattern {
                path,
                pattern,
                detail,
            } => format!("{path}: patrón `{pattern}`: {detail}"),
            Problem::UnknownBaselineVersion { path, version } => {
                format!("{path}: versión {version} desconocida")
            }
            Problem::GitUnavailable(detail) => format!("no se pudo correr git: {detail}"),
            Problem::GitFailed { command, detail } => format!("git {command}: {detail}"),
            Problem::UnknownRule(rule) => format!("no existe la regla `{rule}`"),
            Problem::ExpectedRuleLimit(text) => format!("se esperaba regla=valor y llegó `{text}`"),
            Problem::NotANonNegativeInteger(text) => format!("`{text}` no es un número entero no negativo"),
            Problem::ReadEvent(detail) => format!("no se pudo leer el evento: {detail}"),
            Problem::InvalidEvent(detail) => format!("el evento no es JSON válido: {detail}"),
            Problem::ReadRequest(detail) => format!("no se pudo leer el pedido: {detail}"),
            Problem::InvalidRequest => "el pedido no es JSON válido".to_owned(),
            Problem::UnknownMethod(method) => format!("método desconocido: {method}"),
            Problem::UnknownTool(tool) => format!("herramienta desconocida: {tool}"),
            Problem::InvalidArguments(detail) => format!("argumentos inválidos: {detail}"),
            Problem::ReadEditor(detail) => format!("no se pudo leer del editor: {detail}"),
            Problem::MissingContentLength => "falta el encabezado Content-Length".to_owned(),
            Problem::IncompleteMessage(detail) => format!("mensaje incompleto: {detail}"),
            Problem::InvalidMessage(detail) => format!("el mensaje no es JSON válido: {detail}"),
            Problem::Osv(problem) => security::problem(problem),
        }
    }

    fn notice(&self, notice: &Notice) -> String {
        match notice {
            Notice::Summary(summary) => summarize(summary),
            Notice::Suppressed {
                path,
                line,
                rule,
                reason,
            } => fixes::suppressed(path, *line, *rule, reason),
            Notice::Fix(notice) => fixes::notice(notice),
            Notice::BaselineSaved { count, path } => {
                let saved = counted(*count, "problema guardado", "problemas guardados");
                format!("{saved} en {path}")
            }
            Notice::HookHeader { count } => {
                let problems = counted(*count, "problema", "problemas");
                format!("{problems} en lo que acabás de editar:")
            }
            Notice::OsvDatabaseSaved {
                ecosystem,
                advisories,
                path,
            } => security::database_saved(ecosystem, *advisories, path),
        }
    }
}

fn summarize(summary: &Summary) -> String {
    let Summary {
        files,
        problems,
        known,
        milliseconds,
        deviations,
        suppressed,
    } = *summary;
    let known = if known > 0 {
        format!(" (y {known} en la línea de base)")
    } else {
        String::new()
    };
    let deviations = if deviations > 0 {
        let deviations = counted(deviations, "desvío", "desvíos");
        format!(" · {deviations} del preset en kerbo.toml")
    } else {
        String::new()
    };
    let files = counted(files, "archivo", "archivos");
    let problems = counted(problems, "problema", "problemas");
    format!(
        "{files}, {problems}{known}, {milliseconds} ms{deviations}{}",
        fixes::suppressed_suffix(suppressed)
    )
}

const fn block_unit(unit: BlockUnit) -> &'static str {
    match unit {
        BlockUnit::Tokens => "tokens",
        BlockUnit::SyntaxNodes => "nodos",
    }
}
