use std::collections::HashMap;
use std::collections::hash_map::Entry;

use tree_sitter::Parser;

use crate::language::Language;

#[derive(Default)]
pub struct ParserPool {
    parsers: HashMap<Language, Parser>,
}

impl ParserPool {
    pub fn parser(&mut self, language: Language) -> Result<&mut Parser, String> {
        match self.parsers.entry(language) {
            Entry::Occupied(entry) => Ok(entry.into_mut()),
            Entry::Vacant(entry) => Ok(entry.insert(parser_for(language)?)),
        }
    }
}

pub fn parser_for(language: Language) -> Result<Parser, String> {
    let mut parser = Parser::new();
    if let Some(grammar) = language.grammar() {
        parser.set_language(&grammar).map_err(|error| error.to_string())?;
    }
    Ok(parser)
}
