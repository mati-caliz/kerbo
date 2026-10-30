mod builtins;
mod find;
mod system;
mod text_tools;
pub(super) mod word_shapes;

use tree_sitter::Node;

use crate::shell::commands::{Command, merged_commands};
use crate::shell::context::{Context, Findings};
use crate::shell::words::{literal, literal_or};

const BUILTIN: &str = "builtin";
const PATH_SEPARATOR: char = '/';

// Un comando visto como lo ve `checkCommand` de ShellCheck (`Checks/Commands.hs`): con ruta sólo
// valen los chequeos por nombre de archivo (`Basename`); con `builtin`, los exactos del comando que
// sigue; si no, los dos.
pub struct Invocation<'command, 'tree> {
    command: &'command Command<'tree>,
    exact: Option<String>,
    basename: Option<String>,
    name_word: Node<'tree>,
    arguments: &'command [Node<'tree>],
}

impl<'command, 'tree> Invocation<'command, 'tree> {
    pub const fn node(&self) -> Node<'tree> {
        self.command.node
    }

    pub const fn name_word(&self) -> Node<'tree> {
        self.name_word
    }

    pub const fn arguments(&self) -> &'command [Node<'tree>] {
        self.arguments
    }

    pub fn is_exactly(&self, name: &str) -> bool {
        self.exact.as_deref() == Some(name)
    }

    pub fn has_basename(&self, name: &str) -> bool {
        self.basename.as_deref() == Some(name)
    }
}

pub fn check(node: Node<'_>, context: &Context<'_, '_>, findings: &mut Findings) {
    let Some(command) = Command::of(node) else {
        return;
    };
    let Some(invocation) = invocation(&command, context.source) else {
        return;
    };
    text_tools::check(&invocation, context, findings);
    find::check(&invocation, context, findings);
    builtins::check(&invocation, context, findings);
    system::check(&invocation, context, findings);
}

fn invocation<'command, 'tree>(
    command: &'command Command<'tree>,
    source: &[u8],
) -> Option<Invocation<'command, 'tree>> {
    let first = command.first_word()?;
    let name = if first.is_named() {
        literal(first, source)?
    } else {
        crate::shell::syntax::text(first, source).to_owned()
    };
    let words = merged_commands(&command.words, source)
        .first()
        .copied()
        .unwrap_or_default();
    if name.contains(PATH_SEPARATOR) {
        let basename = name.rsplit(PATH_SEPARATOR).next().unwrap_or_default().to_owned();
        return Some(Invocation {
            command,
            exact: None,
            basename: Some(basename),
            name_word: first,
            arguments: words.get(1..).unwrap_or_default(),
        });
    }
    if name == BUILTIN
        && let Some(selected) = words.get(1)
    {
        return Some(Invocation {
            command,
            exact: Some(literal_or(*selected, source, "")),
            basename: None,
            name_word: *selected,
            arguments: words.get(2..).unwrap_or_default(),
        });
    }
    Some(Invocation {
        command,
        exact: Some(name.clone()),
        basename: Some(name),
        name_word: first,
        arguments: words.get(1..).unwrap_or_default(),
    })
}
