use tree_sitter::Node;

use super::builtin_calls::{Arguments, arguments, keyword_value};
use super::importer::{ImportRequest, ImportStyle, get_or_import_member};
use super::model::Model;
use super::qualified::qualified_name;
use crate::fix::{Fix, TextEdit, TextPoint};
use crate::lint::python::Report;
use crate::lint::python::ast::{
    ARGUMENTS_FIELD, CLASS, FUNCTION, FUNCTION_FIELD, LAMBDA, MODULE, PARENTHESIZED, text, unparenthesized,
};
use crate::rules::PythonRule;

const ASYNC: &str = "async";
const AWAIT: &str = "await";
const INTEGER: &str = "integer";
const FLOAT: &str = "float";
const SCOPES: [&str; 4] = [FUNCTION, CLASS, LAMBDA, MODULE];
const SECONDS_IN_A_DAY: f64 = 86_400.0;
const DIGIT_SEPARATOR: char = '_';
const TRIO_SLEEP: &str = "trio.sleep";
const SLEEP_FOREVER: &str = "sleep_forever";
const EMPTY_CALL: &str = "()";
const AWAIT_PREFIX: &str = "await ";
const ANYIO_SLEEP: &str = "anyio.sleep";
const TRIO_DELAY: &str = "seconds";
const ANYIO_DELAY: &str = "delay";
const MODE: &str = "mode";
const P_WAIT: &str = "os.P_WAIT";
const TRIO_ASYNC_FUNCTIONS: [&str; 23] = [
    "trio.aclose_forcefully",
    "trio.lowlevel.cancel_shielded_checkpoint",
    "trio.lowlevel.checkpoint",
    "trio.lowlevel.checkpoint_if_cancelled",
    "trio.lowlevel.open_process",
    "trio.lowlevel.permanently_detach_coroutine_object",
    "trio.lowlevel.reattach_detached_coroutine_object",
    "trio.lowlevel.temporarily_detach_coroutine_object",
    "trio.lowlevel.wait_readable",
    "trio.lowlevel.wait_task_rescheduled",
    "trio.lowlevel.wait_writable",
    "trio.open_file",
    "trio.open_ssl_over_tcp_listeners",
    "trio.open_ssl_over_tcp_stream",
    "trio.open_tcp_listeners",
    "trio.open_tcp_stream",
    "trio.open_unix_socket",
    "trio.run_process",
    "trio.serve_listeners",
    "trio.serve_ssl_over_tcp",
    "trio.serve_tcp",
    "trio.sleep",
    "trio.sleep_forever",
];
const HTTP_CALLS: [&str; 22] = [
    "urllib.request.urlopen",
    "urllib3.request",
    "httpx.request",
    "httpx.get",
    "httpx.post",
    "httpx.delete",
    "httpx.patch",
    "httpx.put",
    "httpx.head",
    "httpx.connect",
    "httpx.options",
    "httpx.trace",
    "requests.request",
    "requests.get",
    "requests.post",
    "requests.delete",
    "requests.patch",
    "requests.put",
    "requests.head",
    "requests.connect",
    "requests.options",
    "requests.trace",
];
const SUBPROCESS_CREATIONS: [&str; 2] = ["subprocess.Popen", "os.popen"];
const PROCESS_RUNS: [&str; 9] = [
    "os.system",
    "os.posix_spawn",
    "os.posix_spawnp",
    "subprocess.run",
    "subprocess.call",
    "subprocess.check_call",
    "subprocess.check_output",
    "subprocess.getoutput",
    "subprocess.getstatusoutput",
];
const SPAWNS: [&str; 8] = [
    "os.spawnl",
    "os.spawnle",
    "os.spawnlp",
    "os.spawnlpe",
    "os.spawnv",
    "os.spawnve",
    "os.spawnvp",
    "os.spawnvpe",
];

// ASYNC105 (una función async de trio llamada sin `await`), ASYNC116 (`trio.sleep` o `anyio.sleep`
// de más de un día) y, adentro de una función async, ASYNC210 (HTTP que bloquea), ASYNC220 (crear un
// subproceso que bloquea) y ASYNC221 (correr un proceso que bloquea). Avisan en la llamada.
pub fn async_blocking_reports(model: &Model<'_>, source: &[u8], call: Node<'_>) -> Vec<Report> {
    let Some(name) = call
        .child_by_field_name(FUNCTION_FIELD)
        .and_then(|function| qualified_name(model, function, source))
    else {
        return Vec::new();
    };
    let given = arguments(call);
    let mut reports = Vec::new();
    if TRIO_ASYNC_FUNCTIONS.contains(&name.as_str()) && !is_awaited(call) {
        let fix = in_async_function(call, source)
            .then(|| call.child_by_field_name(FUNCTION_FIELD))
            .flatten()
            .map(|function| {
                let at = TextPoint::from_point(function.start_position());
                let awaited = pad_before(AWAIT_PREFIX, function.start_byte(), source);
                Fix::from_edits(
                    source,
                    vec![TextEdit {
                        start: at,
                        end: at,
                        replacement: awaited,
                    }],
                )
                .marked_unsafe()
            });
        reports.push(Report::at(PythonRule::ASYNC105, call, &name).with_fix(fix));
    }
    if is_long_sleep(source, &name, &given) {
        let fix = sleep_forever_fix(model, call, &name, source);
        reports.push(Report::at(PythonRule::ASYNC116, call, "").with_fix(fix));
    }
    if in_async_function(call, source) {
        reports.extend(blocking_rule(model, source, &name, &given).map(|rule| Report::at(rule, call, "")));
    }
    reports
}

fn is_awaited(call: Node<'_>) -> bool {
    std::iter::successors(call.parent(), Node::parent)
        .find(|parent| parent.kind() != PARENTHESIZED)
        .is_some_and(|parent| parent.kind() == AWAIT)
}

fn in_async_function(call: Node<'_>, source: &[u8]) -> bool {
    std::iter::successors(call.parent(), Node::parent)
        .find(|ancestor| SCOPES.contains(&ancestor.kind()))
        .is_some_and(|scope| {
            scope.kind() == FUNCTION && scope.child(0).is_some_and(|first| text(first, source) == ASYNC)
        })
}

// El `pad` de ruff para una inserción: un espacio adelante si queda pegada a una letra.
fn pad_before(content: &str, at: usize, source: &[u8]) -> String {
    let glued = at
        .checked_sub(1)
        .and_then(|before| source.get(before))
        .is_some_and(u8::is_ascii_alphanumeric);
    if glued {
        format!(" {content}")
    } else {
        content.to_owned()
    }
}

// ASYNC116: `trio.sleep(86401)` pasa a `trio.sleep_forever()`; inseguro, como en ruff.
fn sleep_forever_fix(model: &Model<'_>, call: Node<'_>, qualified: &str, source: &[u8]) -> Option<Fix> {
    let (module, _) = qualified.split_once('.')?;
    let function = call.child_by_field_name(FUNCTION_FIELD)?;
    let arguments = call.child_by_field_name(ARGUMENTS_FIELD)?;
    let request = ImportRequest {
        module,
        member: SLEEP_FOREVER,
        style: ImportStyle::ImportFrom,
    };
    let (binding, import) = get_or_import_member(model, source, function, request)?;
    let edits = vec![
        import,
        TextEdit {
            start: TextPoint::from_point(function.start_position()),
            end: TextPoint::from_point(function.end_position()),
            replacement: binding,
        },
        TextEdit {
            start: TextPoint::from_point(arguments.start_position()),
            end: TextPoint::from_point(arguments.end_position()),
            replacement: EMPTY_CALL.to_owned(),
        },
    ];
    Some(Fix::from_edits(source, edits).marked_unsafe())
}

fn is_long_sleep(source: &[u8], name: &str, given: &Arguments<'_>) -> bool {
    let keyword = match name {
        TRIO_SLEEP => TRIO_DELAY,
        ANYIO_SLEEP => ANYIO_DELAY,
        _ => return false,
    };
    if given.positional.len().saturating_add(given.keywords.len()) != 1 {
        return false;
    }
    let Some(delay) = keyword_value(given, keyword, source)
        .or_else(|| given.positional.first().copied())
        .map(unparenthesized)
    else {
        return false;
    };
    if !matches!(delay.kind(), INTEGER | FLOAT) {
        return false;
    }
    let written: String = text(delay, source)
        .chars()
        .filter(|digit| *digit != DIGIT_SEPARATOR)
        .collect();
    written
        .parse::<f64>()
        .is_ok_and(|seconds| seconds > SECONDS_IN_A_DAY)
}

fn blocking_rule(model: &Model<'_>, source: &[u8], name: &str, given: &Arguments<'_>) -> Option<PythonRule> {
    if HTTP_CALLS.contains(&name) {
        return Some(PythonRule::ASYNC210);
    }
    if SUBPROCESS_CREATIONS.contains(&name) {
        return Some(PythonRule::ASYNC220);
    }
    if PROCESS_RUNS.contains(&name) {
        return Some(PythonRule::ASYNC221);
    }
    if !SPAWNS.contains(&name) {
        return None;
    }
    let waits = keyword_value(given, MODE, source)
        .or_else(|| given.positional.first().copied())
        .is_none_or(|mode| qualified_name(model, unparenthesized(mode), source).as_deref() == Some(P_WAIT));
    Some(if waits {
        PythonRule::ASYNC221
    } else {
        PythonRule::ASYNC220
    })
}
