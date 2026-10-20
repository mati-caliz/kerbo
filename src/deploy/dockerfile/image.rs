use super::parser::Instruction;

const DIGEST_SEPARATOR: char = '@';
const TAG_SEPARATOR: char = ':';
const PATH_SEPARATOR: char = '/';
const ALIAS_KEYWORD: &str = "as";
const LOCALHOST: &str = "localhost";
const PLATFORM_FLAG: &str = "platform";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BaseImage {
    pub name: String,
    pub tag: Option<String>,
    pub digest: Option<String>,
    pub alias: Option<String>,
    pub platform: Option<String>,
}

impl BaseImage {
    pub fn parse(instruction: &Instruction) -> Self {
        let mut words = instruction.arguments.split_whitespace();
        let reference = words.next().unwrap_or_default();
        let alias = match (words.next(), words.next()) {
            (Some(keyword), Some(alias)) if keyword.eq_ignore_ascii_case(ALIAS_KEYWORD) => {
                Some(alias.to_owned())
            }
            _ => None,
        };
        let (without_digest, digest) = reference
            .split_once(DIGEST_SEPARATOR)
            .map_or((reference, None), |(image, digest)| {
                (image, Some(digest.to_owned()))
            });
        let tag_start = without_digest.rfind(TAG_SEPARATOR).filter(|&colon| {
            !without_digest
                .get(colon..)
                .unwrap_or_default()
                .contains(PATH_SEPARATOR)
        });
        let (path, tag) = tag_start.map_or((without_digest, None), |colon| {
            (
                without_digest.get(..colon).unwrap_or_default(),
                without_digest.get(colon.saturating_add(1)..).map(str::to_owned),
            )
        });
        Self {
            name: without_registry(path).to_owned(),
            tag,
            digest,
            alias,
            platform: instruction.flag(PLATFORM_FLAG).map(str::to_owned),
        }
    }

    pub fn stage_name(&self) -> &str {
        self.alias.as_deref().unwrap_or(&self.name)
    }
}

fn without_registry(path: &str) -> &str {
    match path.split_once(PATH_SEPARATOR) {
        Some((registry, rest))
            if registry.contains('.') || registry.contains(TAG_SEPARATOR) || registry == LOCALHOST =>
        {
            rest
        }
        Some(_) | None => path,
    }
}
