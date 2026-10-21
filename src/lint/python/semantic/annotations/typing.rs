use tree_sitter::Node;

use super::super::qualified::is_typing;
use super::super::{Mode, Visitor};
use crate::lint::python::ast::{KEYWORD_ARGUMENT, LIST, NAME_FIELD, TUPLE, VALUE_FIELD, named};
use crate::lint::python::semantic::expressions::Expressions;

const TYPING_PREFIXES: [&str; 2] = ["typing.", "typing_extensions."];
const GENERIC_PREFIXES: [&str; 1] = ["collections.abc."];
const GENERICS: [&str; 18] = [
    "builtins.list",
    "builtins.dict",
    "builtins.set",
    "builtins.frozenset",
    "builtins.tuple",
    "builtins.type",
    "collections.deque",
    "collections.defaultdict",
    "collections.OrderedDict",
    "collections.Counter",
    "collections.ChainMap",
    "contextlib.AbstractContextManager",
    "contextlib.AbstractAsyncContextManager",
    "re.Pattern",
    "re.Match",
    "os.PathLike",
    "queue.Queue",
    "weakref.WeakKeyDictionary",
];
const DICTIONARY_SPLAT: &str = "dictionary_splat";
const DICTIONARY: &str = "dictionary";
const PAIR: &str = "pair";
const KEY_FIELD: &str = "key";
const TYPE_KEYWORDS: [&str; 2] = ["bound", "default"];
const FIELD_PAIR: usize = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TypingCall {
    Cast,
    NewType,
    TypeVar,
    NamedTuple,
    TypedDict,
}

pub fn is_generic(name: &str) -> bool {
    TYPING_PREFIXES
        .iter()
        .chain(GENERIC_PREFIXES.iter())
        .any(|prefix| name.starts_with(prefix))
        || GENERICS.contains(&name)
}

fn typing_call(callee: Option<&str>) -> Option<TypingCall> {
    let callee = callee?;
    [
        ("cast", TypingCall::Cast),
        ("TypeForm", TypingCall::Cast),
        ("NewType", TypingCall::NewType),
        ("TypeVar", TypingCall::TypeVar),
        ("NamedTuple", TypingCall::NamedTuple),
        ("TypedDict", TypingCall::TypedDict),
    ]
    .into_iter()
    .find(|(member, _)| is_typing(callee, member))
    .map(|(_, call)| call)
}

pub trait TypingCalls<'tree> {
    fn typing_call(&mut self, arguments: Node<'tree>, callee: Option<&str>);
    fn keyword(&mut self, keyword: Node<'tree>, call: Option<TypingCall>);
    fn named_tuple_fields(&mut self, argument: Node<'tree>);
    fn typed_dict_fields(&mut self, argument: Node<'tree>);
}

impl<'tree> TypingCalls<'tree> for Visitor<'tree> {
    fn typing_call(&mut self, arguments: Node<'tree>, callee: Option<&str>) {
        let (keywords, positional): (Vec<Node<'tree>>, Vec<Node<'tree>>) = named(arguments)
            .into_iter()
            .partition(|argument| matches!(argument.kind(), KEYWORD_ARGUMENT | DICTIONARY_SPLAT));
        let call = typing_call(callee);
        let rest_mode = match call {
            Some(TypingCall::Cast) | None => Mode::Value,
            Some(
                TypingCall::NewType | TypingCall::TypeVar | TypingCall::NamedTuple | TypingCall::TypedDict,
            ) => Mode::TypeDefinition,
        };
        for (index, argument) in positional.into_iter().enumerate() {
            match (call, index) {
                (Some(TypingCall::Cast), 0) => self.expression(argument, Mode::TypeDefinition),
                (Some(_), 0) | (None, _) => self.expression(argument, Mode::Value),
                (Some(TypingCall::NamedTuple), _) => self.named_tuple_fields(argument),
                (Some(TypingCall::TypedDict), _) => self.typed_dict_fields(argument),
                (Some(_), _) => self.expression(argument, rest_mode),
            }
        }
        for keyword in keywords {
            self.keyword(keyword, call);
        }
    }

    fn keyword(&mut self, keyword: Node<'tree>, call: Option<TypingCall>) {
        if keyword.kind() == DICTIONARY_SPLAT {
            let inner = named(keyword).first().copied();
            match (call, inner) {
                (Some(TypingCall::NamedTuple), Some(dictionary)) if dictionary.kind() == DICTIONARY => {
                    self.typed_dict_fields(dictionary);
                }
                _ => self.children(keyword, Mode::Value),
            }
            return;
        }
        let Some(value) = keyword.child_by_field_name(VALUE_FIELD) else {
            return;
        };
        let name = keyword
            .child_by_field_name(NAME_FIELD)
            .map(|name| self.text(name))
            .unwrap_or_default();
        let mode = match call {
            Some(TypingCall::TypeVar) if TYPE_KEYWORDS.contains(&name) => Mode::TypeDefinition,
            Some(TypingCall::NamedTuple | TypingCall::TypedDict) => Mode::TypeDefinition,
            _ => Mode::Value,
        };
        self.expression(value, mode);
    }

    fn named_tuple_fields(&mut self, argument: Node<'tree>) {
        if !matches!(argument.kind(), LIST | TUPLE) {
            self.expression(argument, Mode::Value);
            return;
        }
        for field in named(argument) {
            let parts = named(field);
            let is_pair = matches!(field.kind(), LIST | TUPLE) && parts.len() == FIELD_PAIR;
            let fields = if is_pair { parts } else { vec![field] };
            for (part, mode) in fields.into_iter().zip([Mode::Value, Mode::TypeDefinition]) {
                self.expression(part, mode);
            }
        }
    }

    fn typed_dict_fields(&mut self, argument: Node<'tree>) {
        if argument.kind() != DICTIONARY {
            self.expression(argument, Mode::Value);
            return;
        }
        for item in named(argument) {
            if item.kind() != PAIR {
                self.expression(item, Mode::Value);
                continue;
            }
            if let Some(key) = item.child_by_field_name(KEY_FIELD) {
                self.expression(key, Mode::Value);
            }
            if let Some(value) = item.child_by_field_name(VALUE_FIELD) {
                self.expression(value, Mode::TypeDefinition);
            }
        }
    }
}
