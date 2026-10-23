use super::model::{Alias, AliasComments, Annotated, FromModule, FromParts};

const STAR: &str = "*";

#[derive(Debug)]
pub struct Straight<'source> {
    pub alias: Alias<'source>,
    pub first_index: usize,
    pub atop: Vec<&'source str>,
    pub inline: Vec<&'source str>,
}

// Las tres formas en que ruff agrupa los `from`: los comunes juntos por módulo, cada `x as y` aparte
// y los `*` aparte.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FromKind<'source> {
    Plain,
    Renamed(Alias<'source>),
    Star,
}

#[derive(Debug)]
pub struct FromStatement<'source> {
    pub module: FromModule<'source>,
    pub kind: FromKind<'source>,
    pub first_index: usize,
    pub comments: AliasComments<'source>,
    pub aliases: Vec<(Alias<'source>, AliasComments<'source>)>,
    pub trailing_comma: bool,
}

#[derive(Debug, Default)]
pub struct ImportBlock<'source> {
    pub straight: Vec<Straight<'source>>,
    pub from: Vec<FromStatement<'source>>,
}

// `normalize_imports` de ruff con la configuración por defecto: sin duplicados y con los `from`
// del mismo módulo juntos.
pub fn normalize(imports: Vec<Annotated<'_>>) -> ImportBlock<'_> {
    let mut block = ImportBlock::default();
    for (index, import) in imports.into_iter().enumerate() {
        match import {
            Annotated::Import { names, atop, inline } => {
                if let Some(entry) = names
                    .first()
                    .and_then(|first| block.straight_entry(*first, index))
                {
                    entry.atop.extend(atop);
                    entry.inline.extend(inline);
                }
                for name in names {
                    block.straight_entry(name, index);
                }
            }
            Annotated::From(parts) => block.add_from(parts, index),
        }
    }
    block
}

fn kind_of(alias: Alias<'_>) -> FromKind<'_> {
    if alias.name == STAR {
        FromKind::Star
    } else if alias.asname.is_none() {
        FromKind::Plain
    } else {
        FromKind::Renamed(alias)
    }
}

impl<'source> ImportBlock<'source> {
    fn straight_entry(&mut self, alias: Alias<'source>, index: usize) -> Option<&mut Straight<'source>> {
        if !self.straight.iter().any(|import| import.alias == alias) {
            self.straight.push(Straight {
                alias,
                first_index: index,
                atop: Vec::new(),
                inline: Vec::new(),
            });
        }
        self.straight.iter_mut().find(|import| import.alias == alias)
    }

    fn statement_entry(
        &mut self,
        module: FromModule<'source>,
        kind: FromKind<'source>,
        index: usize,
    ) -> Option<&mut FromStatement<'source>> {
        let matches =
            |statement: &FromStatement<'source>| statement.module == module && statement.kind == kind;
        if !self.from.iter().any(matches) {
            self.from.push(FromStatement {
                module,
                kind,
                first_index: index,
                comments: AliasComments::default(),
                aliases: Vec::new(),
                trailing_comma: false,
            });
        }
        self.from.iter_mut().find(|statement| matches(statement))
    }

    fn add_from(&mut self, parts: FromParts<'source>, index: usize) {
        let FromParts {
            module,
            aliases,
            comments,
            trailing_comma,
        } = parts;
        if let Some(entry) = aliases
            .first()
            .and_then(|first| self.statement_entry(module, kind_of(first.alias), index))
        {
            entry.comments.append(comments);
        }
        for annotated in aliases {
            let Some(entry) = self.statement_entry(module, kind_of(annotated.alias), index) else {
                continue;
            };
            match entry
                .aliases
                .iter_mut()
                .find(|(alias, _)| *alias == annotated.alias)
            {
                Some((_, existing)) => existing.append(annotated.comments),
                None => entry.aliases.push((annotated.alias, annotated.comments)),
            }
            entry.trailing_comma |= trailing_comma;
        }
    }
}
