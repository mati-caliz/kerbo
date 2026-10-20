use super::tests::{analyze_rust, functions, named};
use crate::limits::Limits;
use crate::metrics::FunctionMetrics;
use crate::rules::RuleId;

type Entry = (usize, Option<usize>, Option<usize>, Option<usize>);

fn entries(measured: &[FunctionMetrics]) -> Vec<Entry> {
    measured
        .iter()
        .map(|function| {
            (
                function.line,
                function.lines,
                function.cognitive,
                function.parameters,
            )
        })
        .collect()
}

fn nesting_lines(source: &str) -> Vec<usize> {
    analyze_rust(source, Limits::default().with_rule(RuleId::NestingDepth, Some(1)))
        .nesting
        .iter()
        .map(|finding| finding.line)
        .collect()
}

#[test]
fn counts_what_a_local_macro_hides_in_the_function_that_calls_it() {
    let measured = functions(
        "macro_rules! check {
    ($e:expr) => {
        if $e {
            return Err(());
        }
    };
}

fn uses_check(a: bool, b: bool) -> Result<(), ()> {
    check!(a);
    check!(b);
    Ok(())
}

macro_rules! try_opt {
    ($e:expr) => {
        match $e {
            Some(v) => v,
            None => return None,
        }
    };
}

fn options(a: Option<u8>, b: Option<u8>) -> Option<u8> {
    let x = try_opt!(a);
    let y = try_opt!(b);
    Some(x + y)
}

fn local_macro(a: u8) -> u8 {
    macro_rules! bump {
        ($v:expr) => {
            if $v > 3 { $v } else { $v + 1 }
        };
    }
    bump!(a) + bump!(a + 1)
}

macro_rules! fallible {
    ($e:expr) => {
        $e?
    };
}

fn questioned(text: &str) -> Result<u8, Error> {
    let first = fallible!(text.parse::<u8>());
    Ok(first)
}

macro_rules! looping {
    ($n:expr) => {{
        let mut total = 0;
        while total < $n {
            total += 1;
        }
        for item in 0..$n {
            total += item;
        }
        total
    }};
}

fn looped(n: u32) -> u32 {
    looping!(n)
}
",
    );
    let cognitive: Vec<(&str, Option<usize>)> =
        ["uses_check", "options", "local_macro", "questioned", "looped"]
            .into_iter()
            .map(|name| (name, named(&measured, name).cognitive))
            .collect();
    assert_eq!(
        cognitive,
        [
            ("uses_check", Some(1)),
            ("options", Some(2)),
            ("local_macro", Some(3)),
            ("questioned", Some(1)),
            ("looped", Some(3)),
        ],
        "el if y el match de la macro suman; sus return se descuentan como los propios"
    );
}

#[test]
fn expands_recursive_macros_and_every_fragment_kind() {
    let measured = functions(
        "macro_rules! count {
    () => { 0 };
    ($head:tt $($tail:tt)*) => { 1 + count!($($tail)*) };
}

fn counted() -> usize {
    count!(a b c)
}

macro_rules! munch {
    (@step $acc:expr;) => { $acc };
    (@step $acc:expr; $head:expr $(, $rest:expr)*) => {
        munch!(@step if $head > $acc { $head } else { $acc }; $($rest),*)
    };
    ($($all:expr),+) => { munch!(@step 0; $($all),+) };
}

fn munched(a: u8, b: u8, c: u8) -> u8 {
    munch!(a, b, c)
}

macro_rules! numbers {
    ($($n:expr)*) => {
        $(
            if $n > 2 {
                let _ = $n;
            }
        )*
    };
}

fn listed() {
    numbers!(1 2 u8::MAX);
}

macro_rules! statements {
    ($($s:stmt;)*) => {
        $($s;)*
    };
}

fn statement_fragments(a: u8) -> u8 {
    statements! {
        let b = if a > 1 { a } else { 0 };
        let c = match b { 0 => 1, _ => 2 };
    }
    a
}

macro_rules! patterns {
    ($v:expr, $($p:pat_param)|+) => {
        matches!($v, $($p)|+)
    };
}

fn pattern_fragments(v: Option<u8>) -> bool {
    patterns!(v, Some(1) | None)
}

macro_rules! bracketed {
    [$($e:expr),*] => { [$(if $e { 1 } else { 0 }),*] };
}

fn brackets(a: bool, b: bool) -> [u8; 2] {
    bracketed![a, b]
}
",
    );
    let cognitive: Vec<Option<usize>> = [
        "counted",
        "munched",
        "listed",
        "statement_fragments",
        "pattern_fragments",
        "brackets",
    ]
    .into_iter()
    .map(|name| named(&measured, name).cognitive)
    .collect();
    assert_eq!(
        cognitive,
        [Some(1), Some(8), Some(4), Some(3), Some(2), Some(3)],
        "un `expr` termina donde dos operandos quedan pegados, como en `$($n:expr)*`"
    );
}

#[test]
fn measures_the_functions_a_macro_builds_where_it_writes_them() {
    let measured = functions(
        "macro_rules! getter {
    ($name:ident, $t:ty) => {
        pub fn $name(value: $t, other: $t) -> $t {
            if value > other { value } else { other }
        }
    };
}

getter!(max_u8, u8);
getter!(max_u16, u16);

macro_rules! unused {
    () => {
        fn never(a: u8) -> u8 {
            a
        }
    };
}

macro_rules! with_body {
    ($name:ident $body:block) => {
        pub fn $name() -> u32 $body
    };
}

with_body!(body_fn {
    let x = 1;
    if x > 0 { 2 } else { 3 }
});

macro_rules! visible {
    ($vis:vis $name:ident) => {
        $vis fn $name(a: u8, b: u8) -> u8 {
            a + b
        }
    };
}

visible!(pub vis_pub);
visible!(vis_private);
",
    );
    assert_eq!(
        entries(&measured),
        [
            (3, Some(1), None, None),
            (3, None, None, Some(2)),
            (22, Some(2), None, None),
            (22, None, None, Some(0)),
            (33, Some(1), None, None),
            (33, None, None, Some(2)),
        ],
        "largo y parámetros en la definición, una vez aunque se invoque dos veces; el largo de un \
         `$body:block` es el del texto de la invocación; una macro que nadie usa no arma nada"
    );
}

#[test]
fn measures_whole_functions_written_inside_an_invocation() {
    let source = "macro_rules! passthrough {
    ($($item:item)*) => { $($item)* };
}

passthrough! {
    pub fn passed(a: u8) -> u8 {
        if a > 1 { { { a } } } else { 0 }
    }
}
";
    assert_eq!(
        entries(&functions(source)),
        [(6, Some(1), Some(2), Some(1))],
        "escrita entera en la invocación, se mide como una función más"
    );
    assert_eq!(nesting_lines(source), [7, 7], "el bloque del if y el del else");
}

#[test]
fn takes_the_impl_around_the_invocation_into_account() {
    let measured = functions(
        "struct Holder(u8);

macro_rules! methods {
    () => {
        pub fn first(&self, a: u8) -> u8 {
            self.0 + a
        }
    };
}

impl Holder {
    methods!();
}

trait Doubler {
    fn double(&self, a: u8, b: u8) -> u8;
}

macro_rules! doubler {
    () => {
        fn double(&self, a: u8, b: u8) -> u8 {
            a + b + self.0
        }
    };
}

impl Doubler for Holder {
    doubler!();
}
",
    );
    assert_eq!(
        entries(&measured),
        [
            (5, Some(1), None, None),
            (5, None, None, Some(2)),
            (16, None, None, Some(3)),
            (21, Some(1), None, None),
        ],
        "invocada en la implementación de un trait, la función no cuenta parámetros"
    );
}

#[test]
fn measures_only_the_closures_written_in_the_invocation() {
    let measured = functions(
        "macro_rules! run {
    ($f:expr) => {
        ($f)(1)
    };
}

fn closure_user() -> u8 {
    run!(|x: u8| if x > 0 { x } else { 0 })
}

macro_rules! make_closure {
    () => {
        |x: u8| if x > 1 { x } else { 0 }
    };
}

fn generated_closure() -> u8 {
    (make_closure!())(3)
}
",
    );
    assert_eq!(
        entries(&measured),
        [
            (7, Some(1), Some(1), Some(0)),
            (8, None, Some(2), None),
            (17, Some(1), Some(1), Some(0)),
        ],
        "el closure que arma la macro viene de una expansión y clippy no lo mide"
    );
}
