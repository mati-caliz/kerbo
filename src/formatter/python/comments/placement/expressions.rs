use super::{Placer, handle_bracketed_end_of_line_comment};
use crate::formatter::python::ast::{Expr, NodeId, Span};
use crate::formatter::python::comments::visitor::{DecoratedComment, Placement};
use crate::formatter::python::expression::conditional::find_colons;
use crate::formatter::python::trivia::{BackwardsTokenizer, TokenKind, Tokenizer, contains_line_break};

const SLICE: &str = "slice";

// `a  # comentario\n.b`: antes del punto y en la línea del valor es del valor; cualquier otro
// comentario entre el valor y el atributo queda suelto en el atributo.
fn handle_attribute_comment(
    placer: &Placer<'_>,
    comment: &DecoratedComment,
    (value, attr): (NodeId, Span),
) -> Placement {
    if comment.preceding.is_none() {
        return Placement::Leading(value);
    }
    let value_end = placer.end(value);
    let last_closing_parenthesis = Tokenizer::starts_at(value_end, placer.source)
        .skip_trivia()
        .take_while(|token| token.kind == TokenKind::RParen)
        .last();
    if last_closing_parenthesis.is_some_and(|parenthesis| comment.span.start < parenthesis.span.start) {
        return Placement::Trailing(value);
    }
    let is_before_the_dot = placer
        .token_after_closing_parentheses(value_end, attr.start)
        .is_some_and(|dot| comment.span.end < dot.span.start);
    if comment.is_end_of_line() && is_before_the_dot {
        return Placement::Trailing(value);
    }
    Placement::Dangling(comment.enclosing)
}

// En `a + b`, lo que está antes del operador es de `a`, y el comentario al final de la línea de
// un operador que está solo en su renglón queda suelto en la operación.
fn handle_trailing_binary_expression_left_or_operator_comment(
    placer: &Placer<'_>,
    comment: &DecoratedComment,
    (left, right): (NodeId, NodeId),
) -> Placement {
    if comment.preceding.is_none() || comment.following.is_none() {
        return Placement::Default;
    }
    let (left_end, right_start) = (placer.end(left), placer.start(right));
    let Some(operator) = placer.token_after_closing_parentheses(left_end, right_start) else {
        return Placement::Default;
    };
    let operator_offset = operator.span.start;
    if comment.span.end < operator_offset {
        return Placement::Trailing(left);
    }
    let is_operator_on_its_own_line = contains_line_break(placer.source, left_end, operator_offset)
        && contains_line_break(placer.source, operator_offset, right_start);
    if comment.is_end_of_line() && is_operator_on_its_own_line {
        Placement::Dangling(comment.enclosing)
    } else {
        Placement::Default
    }
}

fn handle_trailing_binary_like_comment(placer: &Placer<'_>, comment: &DecoratedComment) -> Placement {
    let (Some(left_operand), Some(right_operand)) = (comment.preceding, comment.following) else {
        return Placement::Default;
    };
    let operator =
        placer.token_after_closing_parentheses(placer.end(left_operand), placer.start(right_operand));
    if operator.is_some_and(|operator| comment.span.end < operator.span.start) {
        Placement::Trailing(left_operand)
    } else {
        Placement::Default
    }
}

// `not  # comentario\nx` pasa a `not x  # comentario`: queda suelto en la operación.
fn handle_unary_op_comment(placer: &Placer<'_>, comment: &DecoratedComment, operand: NodeId) -> Placement {
    let operand_start = placer.start(operand);
    let up_to = placer
        .tokens(placer.start(comment.enclosing), operand_start)
        .skip(1)
        .find(|token| token.kind == TokenKind::LParen)
        .map_or(operand_start, |parenthesis| parenthesis.span.start);
    if comment.span.end < up_to && comment.is_end_of_line() {
        Placement::Dangling(comment.enclosing)
    } else {
        Placement::Default
    }
}

fn handle_named_expr_comment(placer: &Placer<'_>, comment: &DecoratedComment) -> Placement {
    let (Some(target), Some(value)) = (comment.preceding, comment.following) else {
        return Placement::Default;
    };
    let colon_equal = placer.token_after_closing_parentheses(placer.end(target), placer.start(value));
    if colon_equal.is_some_and(|colon_equal| comment.span.end < colon_equal.span.start) {
        Placement::Trailing(target)
    } else {
        Placement::Dangling(comment.enclosing)
    }
}

fn dangling_unless_parenthesized(placer: &Placer<'_>, comment: &DecoratedComment, start: usize) -> Placement {
    if placer.has_token((start, comment.span.start), TokenKind::LParen) {
        Placement::Default
    } else {
        Placement::Dangling(comment.enclosing)
    }
}

// Los comentarios entre `lambda` y los parámetros, y entre los parámetros y el cuerpo, quedan
// sueltos en la lambda, salvo que estén adentro de los paréntesis del cuerpo.
fn handle_lambda_comment(
    placer: &Placer<'_>,
    comment: &DecoratedComment,
    (parameters, body): (Option<NodeId>, NodeId),
) -> Placement {
    let body_start = placer.start(body);
    let Some(parameters) = parameters else {
        if comment.span.start < body_start {
            return dangling_unless_parenthesized(placer, comment, placer.start(comment.enclosing));
        }
        return Placement::Default;
    };
    if comment.span.start < placer.start(parameters) {
        return if comment.is_own_line() {
            Placement::Leading(parameters)
        } else {
            Placement::Dangling(comment.enclosing)
        };
    }
    let parameters_end = placer.end(parameters);
    if parameters_end < comment.span.start && comment.span.start < body_start {
        return dangling_unless_parenthesized(placer, comment, parameters_end);
    }
    Placement::Default
}

// `**  # comentario\nvalor` en un diccionario: el comentario es del valor.
fn handle_dict_unpacking_comment(placer: &Placer<'_>, comment: &DecoratedComment) -> Placement {
    let Some(following) = comment.following else {
        return Placement::Default;
    };
    let preceding_end = comment.preceding.map_or_else(
        || placer.start(comment.enclosing),
        |preceding| placer.end(preceding),
    );
    let follows_the_stars = placer
        .tokens(preceding_end, comment.span.start)
        .skip_while(|token| token.kind == TokenKind::RParen)
        .any(|token| token.kind == TokenKind::DoubleStar);
    if follows_the_stars {
        Placement::Leading(following)
    } else {
        Placement::Default
    }
}

fn handle_key_value_comment(placer: &Placer<'_>, comment: &DecoratedComment) -> Placement {
    let (Some(following), Some(preceding)) = (comment.following, comment.preceding) else {
        return Placement::Default;
    };
    let between = (placer.end(preceding), placer.start(following));
    if placer.has_token(between, TokenKind::Colon) {
        Placement::Dangling(comment.enclosing)
    } else {
        Placement::Default
    }
}

fn handle_call_comment(placer: &Placer<'_>, comment: &DecoratedComment) -> Placement {
    let is_between_function_and_arguments =
        comment
            .preceding
            .zip(comment.following)
            .is_some_and(|(preceding, following)| {
                placer.end(preceding) < comment.span.start && comment.span.end < placer.start(following)
            });
    if comment.is_own_line() && is_between_function_and_arguments {
        Placement::Dangling(comment.enclosing)
    } else {
        Placement::Default
    }
}

// `a if  # comentario\nb else c`: lo que sigue al `if` o al `else` en su línea es de la
// expresión que viene después.
fn handle_expr_if_comment(
    placer: &Placer<'_>,
    comment: &DecoratedComment,
    (test, body, orelse): (NodeId, NodeId, NodeId),
) -> Placement {
    if comment.is_own_line() {
        return Placement::Default;
    }
    let comment_start = comment.span.start;
    let is_after = |keyword_start: usize, keyword_end: usize, node: NodeId| {
        placer
            .token_after_closing_parentheses(keyword_start, keyword_end)
            .is_some_and(|keyword| keyword.span.start < comment_start && comment_start < placer.start(node))
    };
    if is_after(placer.end(body), placer.start(test), test) {
        return Placement::Leading(test);
    }
    if is_after(placer.end(test), placer.start(orelse), orelse) {
        return Placement::Leading(orelse);
    }
    Placement::Default
}

fn handle_starred_comment(placer: &Placer<'_>, comment: &DecoratedComment) -> Placement {
    let before = (placer.start(comment.enclosing), comment.span.start);
    if comment.following.is_some() && !placer.has_token(before, TokenKind::LParen) {
        Placement::Leading(comment.enclosing)
    } else {
        Placement::Default
    }
}

#[derive(Debug, Clone, Copy)]
enum SliceSection {
    Bound(NodeId),
    Empty,
    Unknown,
}

// A qué parte del slice pertenece el comentario según de qué lado de los dos puntos está.
fn section_of_the_comment(placer: &Placer<'_>, comment: &DecoratedComment, slice: NodeId) -> SliceSection {
    let Some(Expr::Slice { lower, upper, step }) = placer.ast.expr(slice) else {
        return SliceSection::Unknown;
    };
    let Some((first_colon, second_colon)) = find_colons((placer.ast, placer.source), slice, (*lower, *upper))
    else {
        return SliceSection::Unknown;
    };
    let is_after_the_second_colon =
        second_colon.is_some_and(|second_colon| comment.span.start >= second_colon.span.start);
    let bound = if comment.span.start < first_colon.span.start {
        *lower
    } else if is_after_the_second_colon {
        *step
    } else {
        *upper
    };
    bound.map_or(SliceSection::Empty, SliceSection::Bound)
}

fn handle_slice_comments(placer: &Placer<'_>, comment: &DecoratedComment, slice: NodeId) -> Placement {
    let follows_the_bracket =
        BackwardsTokenizer::up_to(comment.span.start, placer.source, placer.comment_ranges)
            .skip_trivia()
            .next()
            .is_some_and(|token| token.kind == TokenKind::LBracket);
    if comment.is_end_of_line() && follows_the_bracket {
        return Placement::Dangling(comment.enclosing);
    }
    match section_of_the_comment(placer, comment, slice) {
        SliceSection::Bound(bound) if comment.span.start < placer.start(bound) => Placement::Leading(bound),
        SliceSection::Bound(bound) => Placement::Trailing(bound),
        SliceSection::Empty => Placement::Dangling(slice),
        SliceSection::Unknown => Placement::Unsupported(SLICE),
    }
}

// `valor[  # comentario`: queda suelto en el subíndice, para seguir en la línea del corchete.
fn handle_subscript_comment(
    placer: &Placer<'_>,
    comment: &DecoratedComment,
    (value, slice): (NodeId, NodeId),
) -> Placement {
    if matches!(placer.ast.expr(slice), Some(Expr::Slice { .. })) {
        return handle_slice_comments(placer, comment, slice);
    }
    let value_end = placer.end(value);
    if comment.is_own_line() || value_end >= comment.span.start {
        return Placement::Default;
    }
    let mut after_the_bracket = placer
        .tokens(value_end, comment.span.start)
        .skip_while(|token| token.kind != TokenKind::LBracket);
    if after_the_bracket.next().is_some() && after_the_bracket.next().is_none() {
        Placement::Dangling(comment.enclosing)
    } else {
        Placement::Default
    }
}

fn handle_dict_comment(placer: &Placer<'_>, comment: &DecoratedComment) -> Placement {
    handle_dict_unpacking_comment(placer, comment)
        .or_else(|| handle_bracketed_end_of_line_comment(placer, comment))
        .or_else(|| handle_key_value_comment(placer, comment))
}

fn handle_dict_comp_comment(placer: &Placer<'_>, comment: &DecoratedComment) -> Placement {
    handle_dict_unpacking_comment(placer, comment)
        .or_else(|| handle_key_value_comment(placer, comment))
        .or_else(|| handle_bracketed_end_of_line_comment(placer, comment))
}

fn handle_operation(placer: &Placer<'_>, comment: &DecoratedComment) -> Placement {
    match placer.ast.expr(comment.enclosing) {
        Some(Expr::BinOp { left, right, .. }) => {
            handle_trailing_binary_expression_left_or_operator_comment(placer, comment, (*left, *right))
        }
        Some(Expr::BoolOp { .. } | Expr::Compare { .. }) => {
            handle_trailing_binary_like_comment(placer, comment)
        }
        Some(Expr::UnaryOp { operand, .. }) => handle_unary_op_comment(placer, comment, *operand),
        Some(Expr::Named { .. }) => handle_named_expr_comment(placer, comment),
        Some(Expr::Lambda { parameters, body }) => {
            handle_lambda_comment(placer, comment, (*parameters, *body))
        }
        Some(Expr::IfExp { test, body, orelse }) => {
            handle_expr_if_comment(placer, comment, (*test, *body, *orelse))
        }
        Some(Expr::Starred { .. }) => handle_starred_comment(placer, comment),
        Some(_) | None => Placement::Default,
    }
}

pub fn handle(placer: &Placer<'_>, comment: &DecoratedComment) -> Placement {
    match placer.ast.expr(comment.enclosing) {
        Some(Expr::Attribute { value, attr }) => handle_attribute_comment(placer, comment, (*value, *attr)),
        Some(Expr::Dict { .. }) => handle_dict_comment(placer, comment),
        Some(Expr::DictComp { .. }) => handle_dict_comp_comment(placer, comment),
        Some(Expr::Slice { .. }) => handle_slice_comments(placer, comment, comment.enclosing),
        Some(Expr::Subscript { value, slice }) => handle_subscript_comment(placer, comment, (*value, *slice)),
        Some(Expr::Call { .. }) => handle_call_comment(placer, comment),
        Some(
            Expr::List { .. }
            | Expr::Set { .. }
            | Expr::ListComp { .. }
            | Expr::SetComp { .. }
            | Expr::Tuple {
                parenthesized: true, ..
            }
            | Expr::Generator {
                parenthesized: true, ..
            },
        ) => handle_bracketed_end_of_line_comment(placer, comment),
        Some(_) | None => handle_operation(placer, comment),
    }
}
