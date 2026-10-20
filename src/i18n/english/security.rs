use super::counted;
use crate::i18n::{Advisory, Explanation, OsvProblem, SecretLeak};

pub const COMMITTED_SECRET_DESCRIPTION: &str =
    "A secret (key, token or password) is committed to the repository";
pub const VULNERABLE_DEPENDENCY_DESCRIPTION: &str = "A dependency with a vulnerability published in OSV";

pub const COMMITTED_SECRET: Explanation = Explanation {
    fix: "revoke the secret, take it out of the code and read it from an environment variable or a secrets manager",
    why: "Anything committed can be read by everyone with access to the repository and stays in the history even after it is deleted: treat it as leaked.",
    example: "const key = \"AKIA…\" becomes const key = process.env.AWS_ACCESS_KEY_ID; test data goes as an exception in kerbo.toml with its reason.",
};
pub const VULNERABLE_DEPENDENCY: Explanation = Explanation {
    fix: "upgrade the dependency to a fixed version and regenerate the lockfile",
    why: "A published vulnerability comes with the recipe in plain sight: attackers look for exactly those versions.",
    example: "lodash 4.17.20 in package-lock.json moves to 4.17.21 with npm install lodash@4.17.21; run kerbo check --osv to confirm it.",
};

pub fn secret(leak: &SecretLeak) -> String {
    let SecretLeak {
        kind,
        preview,
        length,
    } = leak;
    if *length == 0 {
        return format!("A file that usually holds private keys ({kind}) is committed");
    }
    if preview.is_empty() {
        let characters = counted(*length, "character", "characters");
        return format!("Possible committed secret ({kind}), {characters} long");
    }
    let characters = counted(*length, "character", "characters");
    format!("Possible committed secret ({kind}): `{preview}…`, {characters} long")
}

pub fn advisory(advisory: &Advisory) -> String {
    let aliases = if advisory.aliases.is_empty() {
        String::new()
    } else {
        format!(" ({})", advisory.aliases.join(", "))
    };
    let fixed = advisory.fixed.as_ref().map_or_else(
        || ". No fixed version yet".to_owned(),
        |version| format!(". Fixed in {version}"),
    );
    format!(
        "`{}` {} has {}{aliases}: {}{fixed}",
        advisory.package, advisory.version, advisory.id, advisory.summary
    )
}

pub fn problem(problem: &OsvProblem) -> String {
    match problem {
        OsvProblem::Unavailable(detail) => format!("could not query OSV: {detail}"),
        OsvProblem::InvalidResponse(detail) => format!("OSV returned something unexpected: {detail}"),
        OsvProblem::DatabaseMissing(path) => {
            format!("the OSV database is missing at {path}: download it with `kerbo osv-download`")
        }
    }
}

pub fn database_saved(ecosystem: &str, advisories: usize, path: &str) -> String {
    let saved = counted(advisories, "advisory", "advisories");
    format!("{ecosystem}: {saved} saved in {path}")
}
