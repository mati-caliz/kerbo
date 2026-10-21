use super::super::Violation;
use super::super::sql;

const STATEMENT_END: &str = ";\n";
const BEGIN: &str = "BEGIN";
const COMMIT: &str = "COMMIT";
const FIRST_VERSION: &str = "INSERT INTO alembic_version (version_num) VALUES ('head')";
const NEXT_VERSION: &str =
    "UPDATE alembic_version SET version_num='head' WHERE alembic_version.version_num = 'base'";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Origin {
    Call { line: usize, column: usize },
    Execute { first_line: usize, script_line: usize },
    Synthetic,
}

#[derive(Debug)]
pub struct Script {
    text: String,
    origins: Vec<Origin>,
}

impl Script {
    pub fn new() -> Self {
        let mut script = Self {
            text: String::new(),
            origins: Vec::new(),
        };
        script.push(BEGIN, Origin::Synthetic);
        script
    }

    pub fn push(&mut self, sql: &str, origin: Origin) {
        let script_line = self.origins.len().saturating_add(1);
        let origin = match origin {
            Origin::Execute { first_line, .. } => Origin::Execute {
                first_line,
                script_line,
            },
            Origin::Call { .. } | Origin::Synthetic => origin,
        };
        let statement = format!("{sql}{STATEMENT_END}");
        let lines = statement.lines().count().max(1);
        self.origins.extend(std::iter::repeat_n(origin, lines));
        self.text.push_str(&statement);
    }

    pub fn leave_transaction(&mut self) {
        self.push(COMMIT, Origin::Synthetic);
    }

    pub fn enter_transaction(&mut self) {
        self.push(BEGIN, Origin::Synthetic);
    }

    pub fn violations(mut self, first_revision: bool) -> Vec<Violation> {
        self.push(
            if first_revision {
                FIRST_VERSION
            } else {
                NEXT_VERSION
            },
            Origin::Synthetic,
        );
        self.push(COMMIT, Origin::Synthetic);
        sql::violations(&self.text, 1)
            .into_iter()
            .filter_map(|mut violation| {
                let origin = self.origins.get(violation.line.checked_sub(1)?)?;
                match *origin {
                    Origin::Call { line, column } => {
                        violation.line = line;
                        violation.column = column;
                    }
                    Origin::Execute {
                        first_line,
                        script_line,
                    } => {
                        violation.line =
                            first_line.saturating_add(violation.line.saturating_sub(script_line));
                    }
                    Origin::Synthetic => return None,
                }
                Some(violation)
            })
            .collect()
    }
}
