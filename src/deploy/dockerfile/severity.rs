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
        | DockerfileRule::Dl4004
        | DockerfileRule::Dl3011
        | DockerfileRule::Dl3043
        | DockerfileRule::Dl3044 => Severity::Error,
        DockerfileRule::Dl3009
        | DockerfileRule::Dl3015
        | DockerfileRule::Dl3019
        | DockerfileRule::Dl3047
        | DockerfileRule::Dl3059
        | DockerfileRule::Dl3066
        | DockerfileRule::Dl3001
        | DockerfileRule::Dl3010
        | DockerfileRule::Dl3060
        | DockerfileRule::Dl3048 => Severity::Info,
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
        | DockerfileRule::Dl4006
        | DockerfileRule::Dl3028
        | DockerfileRule::Dl3030
        | DockerfileRule::Dl3033
        | DockerfileRule::Dl3034
        | DockerfileRule::Dl3035
        | DockerfileRule::Dl3036
        | DockerfileRule::Dl3037
        | DockerfileRule::Dl3038
        | DockerfileRule::Dl3040
        | DockerfileRule::Dl3041
        | DockerfileRule::Dl3046
        | DockerfileRule::Dl3063
        | DockerfileRule::Dl3065
        | DockerfileRule::Dl4005 => Severity::Warning,
    }
}
