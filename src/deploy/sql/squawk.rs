mod bans;
mod constraints;
mod duplicates;
mod names;
mod objects;
mod robust;
mod statement;
mod transactions;
mod types;
mod volatility;

use super::lexer::Token;
use crate::deploy::Violation;

pub fn violations(statements: &[&[Token]]) -> Vec<Violation> {
    let mut found = Vec::new();
    types::violations(statements, &mut found);
    constraints::violations(statements, &mut found);
    objects::violations(statements, &mut found);
    transactions::violations(statements, &mut found);
    robust::violations(statements, &mut found);
    bans::violations(statements, &mut found);
    duplicates::violations(statements, &mut found);
    found
}

#[cfg(test)]
mod tests;
