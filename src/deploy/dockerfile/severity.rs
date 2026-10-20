use crate::rules::DockerfileRule;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warning,
    Info,
}

pub const fn severity(rule: DockerfileRule) -> Severity {
    match rule {
        DockerfileRule::Dl3000
        | DockerfileRule::Dl3004
        | DockerfileRule::Dl3012
        | DockerfileRule::Dl3020
        | DockerfileRule::Dl3021
        | DockerfileRule::Dl3023
        | DockerfileRule::Dl3024
        | DockerfileRule::Dl3061
        | DockerfileRule::Dl4000
        | DockerfileRule::Dl4004 => Severity::Error,
        DockerfileRule::Dl3009
        | DockerfileRule::Dl3015
        | DockerfileRule::Dl3019
        | DockerfileRule::Dl3047
        | DockerfileRule::Dl3059
        | DockerfileRule::Dl3066 => Severity::Info,
        DockerfileRule::Dl3002
        | DockerfileRule::Dl3003
        | DockerfileRule::Dl3006
        | DockerfileRule::Dl3007
        | DockerfileRule::Dl3008
        | DockerfileRule::Dl3013
        | DockerfileRule::Dl3014
        | DockerfileRule::Dl3016
        | DockerfileRule::Dl3018
        | DockerfileRule::Dl3022
        | DockerfileRule::Dl3025
        | DockerfileRule::Dl3027
        | DockerfileRule::Dl3029
        | DockerfileRule::Dl3032
        | DockerfileRule::Dl3042
        | DockerfileRule::Dl3045
        | DockerfileRule::Dl3062
        | DockerfileRule::Dl3064
        | DockerfileRule::Dl3067
        | DockerfileRule::Dl4001
        | DockerfileRule::Dl4003
        | DockerfileRule::Dl4006 => Severity::Warning,
    }
}
