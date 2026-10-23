use super::segments::{ForkId, SegmentId};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChoiceKind {
    And,
    Or,
    Nullish,
    Test,
    Loop,
}

#[derive(Debug)]
pub struct Choice {
    pub kind: ChoiceKind,
    pub forks_as_result: bool,
    pub true_fork: ForkId,
    pub false_fork: ForkId,
    pub nullish_fork: ForkId,
    pub processed: bool,
}

#[derive(Debug)]
pub struct Break {
    pub breakable: bool,
    pub label: Option<String>,
    pub broken: ForkId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoopKind {
    While,
    DoWhile,
    For,
    ForIn,
}

#[derive(Debug)]
pub struct Loop {
    pub kind: LoopKind,
    pub label: Option<String>,
    pub broken: ForkId,
    pub test: Option<bool>,
    pub continue_destination: Option<Vec<SegmentId>>,
    pub continue_fork: Option<ForkId>,
    pub entry: Vec<SegmentId>,
    pub end_of_init: Vec<SegmentId>,
    pub test_segments: Option<Vec<SegmentId>>,
    pub end_of_test: Option<Vec<SegmentId>>,
    pub update_segments: Option<Vec<SegmentId>>,
    pub end_of_update: Option<Vec<SegmentId>>,
    pub before_left: Vec<SegmentId>,
    pub left_segments: Vec<SegmentId>,
    pub end_of_left: Vec<SegmentId>,
}

// Lo que el `switch` recuerda del `default` para unirlo con el último caso si no va al final.
#[derive(Debug, Default)]
pub struct DefaultCase {
    pub found_empty: bool,
    pub is_last: bool,
}

#[derive(Debug, Default)]
pub struct Switch {
    pub has_case: bool,
    pub default_segments: Option<Vec<SegmentId>>,
    pub default_body: Option<Vec<SegmentId>>,
    pub default_case: DefaultCase,
    pub fork_count: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TryPosition {
    Try,
    Catch,
    Finally,
}

#[derive(Debug)]
pub struct Try {
    pub has_finalizer: bool,
    pub position: TryPosition,
    pub returned: Option<ForkId>,
    pub thrown: ForkId,
}
