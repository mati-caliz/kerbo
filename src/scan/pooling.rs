use std::path::{Path, PathBuf};

use crate::analysis::FileAnalysis;
use crate::duplicates::{PooledFile, find_duplicates};
use crate::language::Stack;
use crate::limits::Limits;
use crate::rules::{Diagnostic, FileIdentity, RuleId};

pub fn pooled_file(
    identity: &FileIdentity,
    analysis: &mut FileAnalysis,
    limits: &Limits,
) -> Option<PooledFile> {
    Some(PooledFile {
        identity: identity.clone(),
        limit: limits.get(RuleId::DuplicateCode)?,
        units: analysis.duplicate_units.take()?,
    })
}

type PoolScope = (Stack, Option<PathBuf>);

pub fn duplicate_diagnostics(pooled: Vec<(Stack, PooledFile)>) -> Vec<Diagnostic> {
    let mut by_scope: Vec<(PoolScope, Vec<PooledFile>)> = Vec::new();
    for (stack, file) in pooled {
        let scope = (stack, file.package_scope().map(Path::to_path_buf));
        match by_scope.iter_mut().find(|(known, _)| *known == scope) {
            Some((_, files)) => files.push(file),
            None => by_scope.push((scope, vec![file])),
        }
    }
    by_scope
        .into_iter()
        .flat_map(|(_, files)| find_duplicates(files))
        .collect()
}
