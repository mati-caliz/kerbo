#![cfg(unix)]

use std::fs;
use std::path::PathBuf;

use super::{Curl, Transport};
use crate::test_support::TempTree;

const SHELL: &str = "sh";
const FAKE_CURL: &str = r#"#!/bin/sh
printf '%s\n' "$@" > "$(dirname "$0")/arguments"
input="$(cat)"
case " $* " in
  *" --config "*)
    printf '%s\n' "$input" | sed -n 's/^output = "\(.*\)"$/\1/p' | while read -r target; do
      printf '{"id":"%s"}' "$(basename "$target")" > "$target"
    done ;;
  *" --output "*)
    previous=""
    for argument in "$@"; do
      if [ "$previous" = "--output" ]; then printf 'zip' > "$argument"; fi
      previous="$argument"
    done ;;
  *) printf 'respuesta a %s' "$input" ;;
esac
"#;
const FAILING_CURL: &str = "#!/bin/sh\necho 'curl: (6) no se resolvió el host' >&2\nexit 6\n";

fn fake(tree: &TempTree, name: &str, script: &str) -> PathBuf {
    tree.write(name, script);
    tree.path().join(name)
}

#[test]
fn talks_to_osv_through_curl_with_timeouts() {
    let tree = TempTree::new("osv-curl");
    let curl = Curl::with_program(SHELL, vec![fake(&tree, "curl", FAKE_CURL).into_os_string()]);
    let answer = curl
        .post_json("https://api.osv.dev/v1/querybatch", b"{}")
        .expect("el post funciona");
    assert_eq!(answer, b"respuesta a {}");
    let arguments = fs::read_to_string(tree.path().join("arguments")).expect("quedaron los argumentos");
    assert!(arguments.contains("--max-time") && arguments.contains("--connect-timeout"));
    let bodies = curl
        .get_all(&["https://a/1".to_owned(), "https://a/2".to_owned()])
        .expect("los get funcionan");
    assert_eq!(
        bodies,
        [b"{\"id\":\"0.json\"}".to_vec(), b"{\"id\":\"1.json\"}".to_vec()]
    );
    assert_eq!(
        curl.get_all(&[]).expect("sin pedidos no corre nada"),
        Vec::<Vec<u8>>::new()
    );
    let destination = tree.path().join("all.zip");
    curl.download("https://a/all.zip", &destination)
        .expect("la descarga funciona");
    assert_eq!(fs::read(&destination).expect("se bajó"), b"zip");
}

#[test]
fn reports_what_curl_says_when_it_fails() {
    let tree = TempTree::new("osv-curl-failing");
    let curl = Curl::with_program(SHELL, vec![fake(&tree, "curl", FAILING_CURL).into_os_string()]);
    let failure = curl
        .post_json("https://api.osv.dev", b"{}")
        .expect_err("curl falla");
    assert!(failure.contains("no se resolvió"), "{failure}");
    let missing = Curl::with_program(tree.path().join("no-existe"), Vec::new());
    assert!(missing.get_all(&["https://a/1".to_owned()]).is_err());
    assert_eq!(Curl::default().program, "curl");
}
