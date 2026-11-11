use super::deferred::{IndexColumn, IndexKind, Kind, Statement, include_clause, nulls_clause};
use super::expression::Expression;
use super::model::{Constraint, ConstraintKind, Index, Model};
use super::schema::{columns_of, quoted_columns};

pub fn constraint_definition(model: &Model, constraint: &Constraint) -> Option<String> {
    match &constraint.kind {
        ConstraintKind::Unique { by_index: true, .. } => None,
        ConstraintKind::Unique {
            fields,
            nulls_distinct,
            deferrable,
            ..
        } => Some(format!(
            "UNIQUE{} ({}){}",
            nulls_clause(*nulls_distinct),
            quoted_columns(&columns_of(model, fields)),
            deferrable.unwrap_or_default()
        )),
        ConstraintKind::Check { condition } => Some(format!("CHECK ({})", condition.render(model))),
        ConstraintKind::Exclusion {
            expressions,
            index_type,
            condition,
            include,
            deferrable,
        } => {
            let excluded: Vec<String> = expressions
                .iter()
                .map(|(expression, operator)| {
                    format!("{} WITH {operator}", expression.index_column(model).render())
                })
                .collect();
            let condition = condition
                .as_ref()
                .map(|condition| format!(" WHERE ({})", condition.render(model)))
                .unwrap_or_default();
            Some(format!(
                "EXCLUDE USING {index_type} ({}){}{condition}{}",
                excluded.join(", "),
                include_clause(&columns_of(model, include)),
                deferrable.unwrap_or_default()
            ))
        }
    }
}

pub fn named_index(model: &Model, index: &Index, kind: IndexKind) -> Statement {
    let columns = if index.fields.is_empty() {
        index
            .expressions
            .iter()
            .map(|expression| expression.index_column(model))
            .collect()
    } else {
        index
            .fields
            .iter()
            .enumerate()
            .map(|(position, entry)| IndexColumn {
                operator_class: index.opclasses.get(position).cloned(),
                ..IndexColumn::parse(entry, |name| model.column_of(name))
            })
            .collect()
    };
    Statement {
        table: model.table(),
        columns,
        kind: Kind::Named {
            name: index.name.clone(),
            kind,
            condition: index.condition.as_ref().map(|condition| condition.render(model)),
            nulls_distinct: None,
            method: index.method,
            storage: index.storage.clone(),
            include: columns_of(model, &index.include),
        },
    }
}

pub fn unique_index(model: &Model, constraint: &Constraint) -> Statement {
    let (fields, expressions, include, condition, opclasses, nulls) = match &constraint.kind {
        ConstraintKind::Unique {
            fields,
            expressions,
            include,
            condition,
            opclasses,
            nulls_distinct,
            ..
        } => (
            fields.clone(),
            expressions.clone(),
            include.clone(),
            condition.clone(),
            opclasses.clone(),
            *nulls_distinct,
        ),
        ConstraintKind::Check { .. } | ConstraintKind::Exclusion { .. } => {
            (Vec::new(), Vec::new(), Vec::new(), None, Vec::new(), None)
        }
    };
    let expressions = if fields.is_empty() && expressions.is_empty() {
        vec![Expression::Opaque]
    } else {
        expressions
    };
    let mut statement = named_index(
        model,
        &Index {
            name: constraint.name.clone(),
            fields,
            expressions,
            include,
            condition,
            method: None,
            opclasses,
            storage: Vec::new(),
        },
        IndexKind::Unique,
    );
    if let Kind::Named { nulls_distinct, .. } = &mut statement.kind {
        *nulls_distinct = nulls;
    }
    statement
}
