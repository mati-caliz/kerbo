use std::collections::HashSet;
use std::fs;
use std::path::Path;
use std::sync::LazyLock;

use ignore::WalkBuilder;
use regex::Regex;

use crate::lint::taint::responses::template_of;
use crate::lint::taint::{TaintMethod, TaintSink};

const PATH_SEPARATOR: char = '/';
const WINDOWS_SEPARATOR: char = '\\';
const EXTENSION_SEPARATOR: char = '.';
const NODE_MODULES: &str = "node_modules";
const EXTENSIONS: &[&str] = &[
    "html",
    "htm",
    "jinja",
    "jinja2",
    "j2",
    "ejs",
    "hbs",
    "handlebars",
    "mustache",
    "pug",
];
// `{% autoescape false %}` en Jinja: todo lo que sigue sale crudo.
const AUTOESCAPE_OFF: &str = "autoescape false";
const VARIABLE_GROUP: &str = "variable";

// Lo que escribe una variable sin escapar: `{{ x|safe }}` en Jinja, `<%- x %>` en EJS, `{{{ x }}}`
// en Handlebars y Mustache, `!{x}` en Pug.
static RAW_OUTPUTS: LazyLock<Vec<Regex>> = LazyLock::new(|| {
    [
        r"\{\{\s*(?P<variable>[A-Za-z_]\w*)[^}]*\|\s*safe\b",
        r"<%-\s*(?P<variable>[A-Za-z_]\w*)",
        r"\{\{\{\s*(?P<variable>[A-Za-z_]\w*)",
        r"!\{\s*(?P<variable>[A-Za-z_]\w*)",
    ]
    .iter()
    .filter_map(|pattern| Regex::new(pattern).ok())
    .collect()
});
static ANY_OUTPUT: LazyLock<Option<Regex>> =
    LazyLock::new(|| Regex::new(r"\{\{\s*(?P<variable>[A-Za-z_]\w*)").ok());

// Las variables que una plantilla escribe sin escapar.
pub fn raw_variables(text: &str) -> HashSet<String> {
    let named = |pattern: &Regex| {
        pattern
            .captures_iter(text)
            .filter_map(|found| found.name(VARIABLE_GROUP))
            .map(|variable| variable.as_str().to_owned())
            .collect::<Vec<_>>()
    };
    let mut raw: HashSet<String> = RAW_OUTPUTS.iter().flat_map(named).collect();
    if text.contains(AUTOESCAPE_OFF)
        && let Some(any) = ANY_OUTPUT.as_ref()
    {
        raw.extend(named(any));
    }
    raw
}

// Las plantillas del proyecto con sus variables crudas, por ruta relativa.
#[derive(Debug, Default)]
pub struct Templates {
    raw: Vec<(String, HashSet<String>)>,
}

impl Templates {
    // Sólo se leen si algún archivo renderiza una: así un proyecto sin plantillas no camina el disco.
    pub fn load<'summary>(root: &Path, mut methods: impl Iterator<Item = &'summary TaintMethod>) -> Self {
        if !methods.any(|method| {
            method
                .sinks
                .iter()
                .any(|sink| template_of(&sink.receiver).is_some())
        }) {
            return Self::default();
        }
        let raw = WalkBuilder::new(root)
            .filter_entry(|entry| entry.file_name() != NODE_MODULES)
            .build()
            .filter_map(Result::ok)
            .filter(|entry| entry.file_type().is_some_and(|kind| kind.is_file()))
            .filter(|entry| {
                entry
                    .path()
                    .extension()
                    .is_some_and(|extension| EXTENSIONS.contains(&extension.to_string_lossy().as_ref()))
            })
            .filter_map(|entry| {
                let text = fs::read_to_string(entry.path()).ok()?;
                let relative = entry.path().strip_prefix(root).unwrap_or_else(|_| entry.path());
                let unified = relative
                    .to_string_lossy()
                    .replace(WINDOWS_SEPARATOR, &PATH_SEPARATOR.to_string());
                Some((unified, text))
            })
            .collect();
        Self::of(raw)
    }

    // Las plantillas ya leídas, con su ruta relativa.
    pub fn of(files: Vec<(String, String)>) -> Self {
        let raw = files
            .into_iter()
            .map(|(path, text)| (path, raw_variables(&text)))
            .filter(|(_, raw)| !raw.is_empty())
            .collect();
        Self { raw }
    }

    // `render_template('pagina.html')` busca `.../pagina.html`; `res.render('pagina')`, también con
    // la extensión de la plantilla.
    fn renders_raw(&self, template: &str, variable: &str) -> bool {
        self.raw.iter().any(|(path, raw)| {
            let name = path
                .rsplit_once(PATH_SEPARATOR)
                .map_or(path.as_str(), |(_, name)| name);
            let without_extension = name
                .split_once(EXTENSION_SEPARATOR)
                .map_or(name, |(stem, _)| stem);
            let matches = path == template
                || path.ends_with(&format!("{PATH_SEPARATOR}{template}"))
                || without_extension == template.rsplit(PATH_SEPARATOR).next().unwrap_or(template);
            matches && raw.contains(variable)
        })
    }

    // Si el sumidero es de una plantilla, si esa variable sale cruda; si no es de plantilla, nada.
    pub fn is_raw(&self, sink: &TaintSink) -> Option<bool> {
        template_of(&sink.receiver).map(|(template, variable)| self.renders_raw(template, variable))
    }
}
