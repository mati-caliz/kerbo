use std::collections::{BTreeSet, HashMap};

use tree_sitter::Node;

use crate::rules::ShellRule;
use crate::shell::commands::Command;
use crate::shell::context::{Context, Findings};
use crate::shell::parameters::is_variable_name;
use crate::shell::rules::analytics::annotations::is_disabled;
use crate::shell::syntax::{descendants, named_children, text};
use crate::shell::word_pieces::{only_literal_text, pieces, unquoted_literal};

const ALIAS: &str = "alias";
const ALIAS_SEPARATOR: char = '=';
const PATH_SEPARATOR: char = '/';
const NOT_LITERAL: &str = "-";
const DEFINITION_RULE: &str = "sc2262";

// SC2262 y SC2263: `checkAliasUsedInSameParsingUnit`.
pub fn check(context: &Context<'_, '_>, findings: &mut Findings) {
    let statements = named_children(context.root);
    let mut start = 0;
    for (index, pair) in statements.windows(2).enumerate() {
        let [current, next] = pair else {
            continue;
        };
        if current.end_position().row != next.start_position().row {
            check_unit(
                statements.get(start..=index).unwrap_or_default(),
                context,
                findings,
            );
            start = index.saturating_add(1);
        }
    }
    check_unit(statements.get(start..).unwrap_or_default(), context, findings);
}

fn record_aliases<'tree>(
    arguments: &[Node<'tree>],
    source: &[u8],
    aliases: &mut HashMap<String, Node<'tree>>,
) {
    for argument in arguments {
        let raw = only_literal_text(&pieces(*argument, source), NOT_LITERAL);
        if let Some((alias, _)) = raw.split_once(ALIAS_SEPARATOR)
            && is_variable_name(alias)
        {
            aliases.entry(alias.to_owned()).or_insert(*argument);
        }
    }
}

fn check_unit(unit: &[Node<'_>], context: &Context<'_, '_>, findings: &mut Findings) {
    let source = context.source;
    let mut aliases: HashMap<String, Node<'_>> = HashMap::new();
    let mut reported = BTreeSet::new();
    for command in unit
        .iter()
        .flat_map(|statement| descendants(*statement))
        .filter_map(Command::of)
    {
        let Some(name) = command
            .first_word()
            .and_then(|first| unquoted_literal(&pieces(first, source)))
        else {
            continue;
        };
        if name == ALIAS {
            record_aliases(command.arguments(), source, &mut aliases);
        } else if !name.contains(PATH_SEPARATOR)
            && let Some(definition) = aliases.get(&name)
            && !is_disabled(*definition, source, DEFINITION_RULE)
        {
            if reported.insert(definition.start_byte()) {
                findings.report(ShellRule::Sc2262, *definition, text(*definition, source));
            }
            findings.report(ShellRule::Sc2263, command.node, &name);
        }
    }
}
