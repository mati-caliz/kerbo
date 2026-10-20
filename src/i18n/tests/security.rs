use crate::i18n::{Advisory, Lang, Message, Notice, SecretLeak};

fn leak(preview: &str, length: usize) -> Message {
    Message::CommittedSecret(SecretLeak {
        kind: "aws-access-token".to_owned(),
        preview: preview.to_owned(),
        length,
    })
}

fn advisory(aliases: &[&str], fixed: Option<&str>) -> Message {
    Message::VulnerableDependency(Box::new(Advisory {
        package: "lodash".to_owned(),
        version: "4.17.20".to_owned(),
        id: "GHSA-35jh-r3h4-6jhm".to_owned(),
        aliases: aliases.iter().map(|alias| (*alias).to_owned()).collect(),
        summary: "Command Injection in lodash".to_owned(),
        fixed: fixed.map(str::to_owned),
    }))
}

pub fn messages() -> Vec<Message> {
    vec![
        leak("AKIA", 20),
        leak("", 8),
        leak("", 0),
        advisory(&["CVE-2021-23337"], Some("4.17.21")),
        advisory(&[], None),
    ]
}

#[test]
fn security_messages_never_show_the_whole_secret() {
    assert_eq!(
        Lang::Spanish.message(&leak("AKIA", 20)),
        "Posible secreto versionado (aws-access-token): `AKIA…`, 20 caracteres"
    );
    assert_eq!(
        Lang::English.message(&leak("", 1)),
        "Possible committed secret (aws-access-token), 1 character long"
    );
    assert_eq!(
        Lang::English.message(&advisory(&["CVE-2021-23337"], Some("4.17.21"))),
        "`lodash` 4.17.20 has GHSA-35jh-r3h4-6jhm (CVE-2021-23337): Command Injection in lodash. Fixed in 4.17.21"
    );
    assert_eq!(
        Lang::Spanish.message(&advisory(&[], None)),
        "`lodash` 4.17.20 tiene GHSA-35jh-r3h4-6jhm: Command Injection in lodash. Todavía no hay una versión arreglada"
    );
    let saved = |advisories| Notice::OsvDatabaseSaved {
        ecosystem: "npm".to_owned(),
        advisories,
        path: "all.zip".to_owned(),
    };
    assert_eq!(
        Lang::English.notice(&saved(1)),
        "npm: 1 advisory saved in all.zip"
    );
    assert_eq!(
        Lang::Spanish.notice(&saved(2)),
        "2 avisos de npm guardados en all.zip"
    );
}
