use super::ModuleIdentity;

fn inside_a_package() -> ModuleIdentity {
    ModuleIdentity {
        path: Some(vec![
            "ipython".to_owned(),
            "terminal".to_owned(),
            "debugger".to_owned(),
        ]),
        name: Some("debugger".to_owned()),
    }
}

fn loose_script() -> ModuleIdentity {
    ModuleIdentity {
        path: None,
        name: Some("gzip".to_owned()),
    }
}

#[test]
fn a_definition_takes_the_package_path_or_the_script_name() {
    assert_eq!(
        inside_a_package().member("Pdb").as_deref(),
        Some("ipython.terminal.debugger.Pdb")
    );
    assert_eq!(loose_script().member("open").as_deref(), Some("gzip.open"));
    assert_eq!(ModuleIdentity::default().member("open"), None);
}

#[test]
fn relative_imports_resolve_only_inside_a_package() {
    let package = inside_a_package();
    assert_eq!(
        package.resolved_import(".embed").as_deref(),
        Some("ipython.terminal.embed")
    );
    assert_eq!(
        package.resolved_import("..core.magic").as_deref(),
        Some("ipython.core.magic")
    );
    assert_eq!(package.resolved_import("....too.far"), None);
    assert_eq!(package.resolved_import("os.path").as_deref(), Some("os.path"));
    assert_eq!(loose_script().resolved_import(".embed"), None);
}
