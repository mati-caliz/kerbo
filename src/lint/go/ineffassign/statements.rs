use super::builder::Builder;
use super::is_zero_initializer;
use crate::lint::go::ast::{
    AssignOp, BranchKind, Expr, ForStmt, IfStmt, RangeStmt, Stmt, walk_expr, walk_stmt, walk_stmts,
};

pub fn statement<'file>(builder: &mut Builder<'file>, statement: &'file Stmt) -> bool {
    match statement {
        Stmt::If(conditional) => if_stmt(builder, conditional),
        Stmt::For(looped) => for_stmt(builder, statement, looped),
        Stmt::Range(ranged) => range_stmt(builder, statement, ranged),
        Stmt::Switch { init, tag, body } => {
            builder.walk_optional(init.as_deref());
            if let Some(tag) = tag {
                walk_expr(builder, tag);
            }
            switch_cases(builder, statement, body);
        }
        Stmt::TypeSwitch { init, assign, body } => {
            builder.walk_optional(init.as_deref());
            walk_stmt(builder, assign);
            switch_cases(builder, statement, body);
        }
        Stmt::Select(cases) => select(builder, statement, cases),
        Stmt::Defer(call) => defer(builder, call),
        Stmt::Labeled {
            label,
            statement: inner,
        } => {
            let destination = builder.new_block(&builder.current_parents());
            let label_target = builder.branch_target(Some(*label));
            let target = builder.gotos.get(&mut builder.graph, label_target);
            builder.graph.set_destination(target, destination);
            builder.label_statement = Some((inner, builder.resolution.object(*label)));
            walk_stmt(builder, inner);
        }
        Stmt::Branch { kind, label } => branch(builder, *kind, *label),
        Stmt::Assign { left, op, right } => assign(builder, left, *op, right),
        Stmt::Decl(decl) => builder.var_decl(decl),
        Stmt::IncDec(target) => match target.unparen_ident() {
            Some(ident) => {
                builder.use_ident(ident);
                builder.assign_ident(ident);
            }
            None => walk_expr(builder, target),
        },
        Stmt::Return(values) => return_stmt(builder, values),
        Stmt::Send { .. } => {
            builder.maybe_panic();
            return true;
        }
        Stmt::Empty
        | Stmt::Expr(_)
        | Stmt::Go(_)
        | Stmt::Block(_)
        | Stmt::Case { .. }
        | Stmt::Comm { .. } => return true,
    }
    false
}

fn if_stmt<'file>(builder: &mut Builder<'file>, conditional: &'file IfStmt) {
    builder.walk_optional(conditional.init.as_ref());
    walk_expr(builder, &conditional.condition);
    let mut before = builder.current_parents();
    builder.new_block(&before);
    walk_stmts(builder, &conditional.body);
    let after_body = builder.current_parents();
    if let Some(alternative) = &conditional.alternative {
        builder.new_block(&before);
        walk_stmt(builder, alternative);
        before = builder.current_parents();
    }
    builder.new_block(&[before, after_body].concat());
}

fn for_stmt<'file>(builder: &mut Builder<'file>, statement: &'file Stmt, looped: &'file ForStmt) {
    let label = builder.statement_label(statement);
    let exit = builder.breaks.push(&mut builder.graph, label);
    let next = builder.continues.push(&mut builder.graph, label);
    builder.walk_optional(looped.init.as_ref());
    let start = builder.new_block(&builder.current_parents());
    if let Some(condition) = &looped.condition {
        walk_expr(builder, condition);
    }
    let condition_block = builder.current_parents();
    builder.new_block(&condition_block);
    walk_stmts(builder, &looped.body);
    let continue_block = builder.new_block(&builder.current_parents());
    builder.graph.set_destination(next, continue_block);
    builder.walk_optional(looped.post.as_ref());
    link_current_to(builder, start);
    let exit_block = builder.new_block(&condition_block);
    builder.graph.set_destination(exit, exit_block);
    builder.breaks.pop();
    builder.continues.pop();
}

fn range_stmt<'file>(builder: &mut Builder<'file>, statement: &'file Stmt, ranged: &'file RangeStmt) {
    let label = builder.statement_label(statement);
    let exit = builder.breaks.push(&mut builder.graph, label);
    let next = builder.continues.push(&mut builder.graph, label);
    walk_expr(builder, &ranged.operand);
    let before = builder.new_block(&builder.current_parents());
    let start = builder.new_block(&[before]);
    if ranged.key.is_some() {
        for bound in [&ranged.key, &ranged.value].into_iter().flatten() {
            match bound.unparen_ident() {
                Some(ident) => builder.assign_ident(ident),
                None => walk_expr(builder, bound),
            }
        }
    }
    walk_stmts(builder, &ranged.body);
    link_current_to(builder, start);
    builder.graph.set_destination(next, before);
    let exit_block = builder.new_block(&[vec![before], builder.current_parents()].concat());
    builder.graph.set_destination(exit, exit_block);
    builder.breaks.pop();
    builder.continues.pop();
}

fn switch_cases<'file>(builder: &mut Builder<'file>, statement: &'file Stmt, cases: &'file [Stmt]) {
    let label = builder.statement_label(statement);
    let exit = builder.breaks.push(&mut builder.graph, label);
    let before = builder.current_parents();
    let mut list = before.clone();
    let mut exits = Vec::new();
    let mut default_block = None;
    let mut fallthrough_block: Option<Vec<usize>> = None;
    for case in cases {
        let Stmt::Case { list: values, body } = case else {
            continue;
        };
        let mut parents = Vec::new();
        if let Some(values) = values {
            list = vec![builder.new_block(&list)];
            for value in values {
                walk_expr(builder, value);
            }
            parents.extend(list.iter().copied());
        }
        parents.extend(fallthrough_block.take().unwrap_or_default());
        builder.new_block(&parents);
        if values.is_none() {
            default_block = builder.current;
        }
        for inner in body {
            walk_stmt(builder, inner);
            if matches!(
                inner,
                Stmt::Branch {
                    kind: BranchKind::Fallthrough,
                    ..
                }
            ) {
                fallthrough_block = Some(builder.current_parents());
            }
        }
        if fallthrough_block.is_none() {
            exits.extend(builder.current_parents());
        }
    }
    match default_block {
        Some(default_block) => {
            for block in &list {
                builder.graph.add_child(*block, default_block);
            }
        }
        None => exits.extend(before),
    }
    let exit_block = builder.new_block(&exits);
    builder.graph.set_destination(exit, exit_block);
    builder.breaks.pop();
}

fn select<'file>(builder: &mut Builder<'file>, statement: &'file Stmt, cases: &'file [Stmt]) {
    let label = builder.statement_label(statement);
    let exit = builder.breaks.push(&mut builder.graph, label);
    for case in cases {
        let Stmt::Comm { communication, .. } = case else {
            continue;
        };
        match communication.as_deref() {
            Some(Stmt::Assign { right, .. }) => {
                if let Some(first) = right.first() {
                    walk_expr(builder, first);
                }
            }
            other => builder.walk_optional(other),
        }
    }
    let before = builder.current_parents();
    let mut exits = Vec::new();
    let mut has_default = false;
    for case in cases {
        builder.new_block(&before);
        walk_stmt(builder, case);
        exits.extend(builder.current_parents());
        has_default = has_default
            || matches!(
                case,
                Stmt::Comm {
                    communication: None,
                    ..
                }
            );
    }
    if !has_default {
        exits.extend(before);
    }
    let exit_block = builder.new_block(&exits);
    builder.graph.set_destination(exit, exit_block);
    builder.breaks.pop();
}

fn defer<'file>(builder: &mut Builder<'file>, call: &'file Expr) {
    if let Expr::Call { function, arguments } = call.unparen() {
        walk_expr(builder, function);
        for argument in arguments {
            walk_expr(builder, argument);
        }
    } else {
        walk_expr(builder, call.unparen());
    }
    if let Some(deferred) = builder.defers.last_mut() {
        *deferred = true;
    }
}

fn branch(builder: &mut Builder<'_>, kind: BranchKind, label: Option<usize>) {
    let label_object = builder.branch_target(label);
    let target = match kind {
        BranchKind::Break => builder.breaks.get(&mut builder.graph, label_object),
        BranchKind::Continue => builder.continues.get(&mut builder.graph, label_object),
        BranchKind::Goto => builder.gotos.get(&mut builder.graph, label_object),
        BranchKind::Fallthrough => return,
    };
    if let Some(source) = builder.current {
        builder.graph.add_source(target, source);
    }
    builder.new_block(&[]);
}

fn assign<'file>(builder: &mut Builder<'file>, left: &'file [Expr], op: AssignOp, right: &'file [Expr]) {
    if op == (AssignOp::Compound { divides: true }) {
        builder.maybe_panic();
    }
    for value in right {
        walk_expr(builder, value);
    }
    for (index, target) in left.iter().enumerate() {
        let Some(ident) = target.unparen_ident() else {
            walk_expr(builder, target);
            continue;
        };
        if matches!(op, AssignOp::Compound { .. }) {
            builder.use_ident(ident);
        }
        let zero = right
            .get(index)
            .is_some_and(|value| is_zero_initializer(value, builder.resolution, builder.file));
        if op == AssignOp::Define && zero {
            builder.use_ident(ident);
        } else {
            builder.assign_ident(ident);
        }
    }
}

fn return_stmt<'file>(builder: &mut Builder<'file>, values: &'file [Expr]) {
    for value in values {
        walk_expr(builder, value);
    }
    let results = builder.results.last().copied().flatten().unwrap_or_default();
    for name in results.iter().flat_map(|field| &field.names) {
        if !values.is_empty() {
            builder.assign_ident(*name);
        }
        builder.use_ident(*name);
    }
    builder.new_block(&[]);
}

fn link_current_to(builder: &mut Builder<'_>, target: usize) {
    if let Some(current) = builder.current {
        builder.graph.add_child(current, target);
    }
}
