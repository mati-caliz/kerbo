use super::catalog::{self, Catalog};
use super::detector::shannon_entropy;
use super::location::LineIndex;
use super::{SecretFinding, find_secrets, path_profile};

const DEFAULT_RULE_COUNT: usize = 223;

fn scan(path: &str, content: &str) -> Vec<SecretFinding> {
    path_profile(path).map_or_else(Vec::new, |profile| find_secrets(content.as_bytes(), &profile))
}

fn kinds(findings: &[SecretFinding]) -> Vec<&str> {
    findings.iter().map(|finding| finding.kind.as_str()).collect()
}

const NOTHING: [&str; 0] = [];

fn encoded(text: &str, each: impl Fn(char) -> String) -> String {
    text.chars().map(each).collect::<Vec<_>>().concat()
}

fn aws_key() -> String {
    ["AKIA", "QYLPMN5HHHFPZAM2"].concat()
}

fn github_token() -> String {
    ["ghp_", "8vFzq2PrX1mL0tK7cJd4nW9bYh3sG5aUeR6i"].concat()
}

#[test]
fn every_default_rule_loads_and_compiles() {
    let shared = catalog::shared().expect("la configuración de gitleaks se lee");
    assert_eq!(shared.rules().len(), DEFAULT_RULE_COUNT);
    for rule in shared.rules() {
        assert!(!rule.has_pattern() || rule.pattern().is_some(), "{}", rule.id);
    }
    assert!(Catalog::parse("rules = 3", "").is_err());
}

#[test]
fn finds_cloud_keys_and_tokens_and_masks_them() {
    let content = format!("aws_key = \"{}\"\ntoken: {}\n", aws_key(), github_token());
    let found = scan("deploy/config.yml", &content);
    assert_eq!(kinds(&found), ["aws-access-token", "github-pat"]);
    let first = &found[0];
    assert_eq!((first.line, first.column), (1, 12));
    assert_eq!(first.preview, "AKIA");
    assert_eq!(first.length, aws_key().len());
    assert_eq!(found[1].line, 2);
}

#[test]
fn finds_private_keys_and_connection_strings() {
    let pem = [
        "-----BEGIN RSA PRIVATE ",
        "KEY-----\nMIIEowIBAAKCAQEAu1SU1LfVLPHCozMxH2Mo4lgOEePzNm0tRgeLezV6ffAt0gunVTLw7onLRnrq0/IzW7yWR7QkrmBL7jTKEn5u+qKhbwKfBstIs+bMY2Zkp18gnTxKLxoS2tFczGkPLPgizskuemMghRniWaoLcyehkd3qqGElvW/VDL5AaWTg0nLVkjRo9z+40RQzuVaE8AkAFmxZzow3x+VJYKdjykkJ0iT9wCS0DRTXu269V264Vf/3jvredZiKRkgwlL9xNAwxXFg0x/XFw005UWVRIkdgcKWTjpBP2dPwVZ4WWC+9aGVd+Gyn1o0CLelf4rEjGoXbAAEgAqeGUxrcIlbjXfbcmwIDAQAB\n-----END RSA PRIVATE KEY-----\n",
    ]
    .concat();
    let connection = ["postgres://admin:", "Zx81kq0Lmn4Tp@db.internal:5432/app"].concat();
    let found = scan("keys/server.pem", &format!("{pem}DATABASE_URL={connection}\n"));
    assert!(kinds(&found).contains(&"private-key"), "{found:?}");
}

#[test]
fn skips_what_gitleaks_skips() {
    let content = format!("aws_key = \"{}\"\n", aws_key());
    assert!(path_profile("node_modules/lib/index.js").is_none());
    assert!(path_profile("go.sum").is_none());
    let allowed_below = format!("{content}// gitleaks:allow\n");
    assert_eq!(scan("src/app.js", &allowed_below).len(), 1);
    let allowed_inline = format!("aws_key = \"{}\" // gitleaks:allow\n", aws_key());
    let placeholders = "password = \"changeme\"\napi_key = \"${API_KEY}\"\n";
    let zipped = format!("PK\u{3}\u{4} {content}");
    for (path, text) in [
        ("src/app.js", allowed_inline.as_str()),
        ("src/app.js", placeholders),
        ("src/app.js", "\u{0}PK\u{3}\u{4}"),
        ("bundle.dat", zipped.as_str()),
        ("src/empty.js", ""),
    ] {
        assert_eq!(kinds(&scan(path, text)), NOTHING, "{path}: {text}");
    }
}

#[test]
fn generic_matches_give_way_to_specific_rules_on_the_same_line() {
    let found = scan("app.env", &format!("GITHUB_TOKEN={}\n", github_token()));
    assert_eq!(kinds(&found), ["github-pat"]);
    let generic = scan("app.env", "client_secret = \"q8Xz2LmP0vR7tK4sW9nB\"\n");
    assert_eq!(kinds(&generic), ["generic-api-key"]);
}

#[test]
fn reports_files_found_by_name_alone() {
    let found = scan("certs/client.p12", "contenido");
    assert_eq!(kinds(&found), ["pkcs12-file"]);
    assert_eq!(found[0].length, 0);
    assert_eq!(found[0].preview, "");
}

#[test]
fn locates_like_gitleaks_even_on_the_last_line() {
    let content = b"a\nbb\ncc";
    let index = LineIndex::new(content);
    let last = index.locate(content, 5..7);
    assert_eq!((last.line, last.column, last.end_line), (3, 1, 3));
    assert_eq!(last.text, 0..7);
    let middle = index.locate(content, 2..4);
    assert_eq!((middle.line, middle.column), (2, 1));
    assert_eq!(middle.text, 1..4);
    let single = LineIndex::new(b"abc");
    assert_eq!(single.locate(b"abc", 1..2).text, 0..3);
}

#[test]
fn finds_secrets_hidden_behind_encodings_like_gitleaks() {
    let base64 = scan(
        "config.txt",
        "intro\nblob: YXdzX2tleSA9ICJBS0lBUVlMUE1ONUhISEZQWkFNMiI=\n",
    );
    assert_eq!(kinds(&base64), ["aws-access-token"]);
    assert_eq!(base64[0].line, 2);
    let hex = encoded(&aws_key(), |character| format!("{:02x}", u32::from(character)));
    assert_eq!(
        kinds(&scan("config.txt", &format!("value: {hex}\n"))),
        ["aws-access-token"]
    );
    let token = format!("token=\"{}\"", github_token());
    let percent = encoded(&token, |character| format!("%{:02x}", u32::from(character)));
    assert_eq!(
        kinds(&scan("query.txt", &format!("q={percent}\n"))),
        ["github-pat"]
    );
    let escaped = encoded(&aws_key(), |character| format!("\\u{:04x}", u32::from(character)));
    assert_eq!(
        kinds(&scan("data.json", &format!("{{\"k\": \"{escaped}\"}}\n"))),
        ["aws-access-token"]
    );
    let points = encoded(&aws_key(), |character| format!("U+{:04X} ", u32::from(character)));
    assert_eq!(
        kinds(&scan("notes.txt", &format!("{points}\n"))),
        ["aws-access-token"]
    );
}

#[test]
fn translates_go_braces_that_rust_would_reject() {
    use super::dialect::to_rust;
    assert_eq!(to_rust(r"^\${(?:[A-Z_]+)}$"), r"^\$\{(?:[A-Z_]+)\}$");
    assert_eq!(to_rust(r"a{2,5}b{3}c{,4}"), r"a{2,5}b{3}c\{,4\}");
    assert_eq!(to_rust(r"[{}\]x][[:alnum:]{]"), r"[{}\]x][[:alnum:]{]");
    assert_eq!(to_rust(r"[^]a]{1}\"), r"[^]a]{1}\");
}

#[test]
fn finds_passwords_in_connection_strings_but_not_placeholders() {
    let connection = ["postgres://admin:", "Zx81kq0Lmn4Tp@db.internal:5432/app"].concat();
    let found = scan("deploy/app.yaml", &format!("url: {connection}\n"));
    assert_eq!(kinds(&found), ["connection-string-password"]);
    assert_eq!(found[0].length, "Zx81kq0Lmn4Tp".len());
    let placeholders =
        "a: mysql://root:${DB_PASSWORD}@db/app\nb: redis://user:password@cache\nc: postgres://u:<pass>@h/d\n";
    assert_eq!(kinds(&scan("deploy/app.yaml", placeholders)), NOTHING);
}

#[test]
fn measures_entropy_like_gitleaks() {
    assert!(shannon_entropy(b"").abs() < f64::EPSILON);
    assert!(shannon_entropy(b"aaaa").abs() < f64::EPSILON);
    assert!((shannon_entropy(b"abcd") - 2.0).abs() < f64::EPSILON);
}
