use super::*;

#[test]
fn reads_the_user_model_the_automatic_key_and_the_numeric_constants() {
    let sources = vec![
        "import os
AUTH_USER_MODEL = \"account.User\"
DEFAULT_AUTO_FIELD = 'django.db.models.BigAutoField'
CODE_LENGTH = 3
    INDENTED = 4
CONCURRENTLY = True
DEBUG = False
CONCURRENTLY = False
DEBUG_TOOLBAR = maybe()
"
        .to_owned(),
    ];
    let settings = Settings::read(&sources);
    assert_eq!(settings.user_model, ModelKey::new("account", "user"));
    assert_eq!(settings.auto_field, Class::BigAuto);
    assert_eq!(settings.number("settings.CODE_LENGTH"), "3");
    assert_eq!(settings.number("settings.INDENTED"), "settings.INDENTED");
    assert_eq!(settings.number("CODE_LENGTH"), "CODE_LENGTH");
    assert_eq!(settings.flag("CONCURRENTLY"), Some(false));
    assert_eq!(settings.flag("DEBUG"), Some(false));
    assert_eq!(settings.flag("DEBUG_TOOLBAR"), None);
}

#[test]
fn reads_chained_and_annotated_assignments() {
    let sources = vec![
        "SOCIAL_AUTH_USER_MODEL = AUTH_USER_MODEL = \"sentry.User\"
PAGE_SIZE: int = 50
URL = \"http://host/?a=b\"
IS_DEBUG == True
"
        .to_owned(),
    ];
    let settings = Settings::read(&sources);
    assert_eq!(settings.user_model, ModelKey::new("sentry", "User"));
    assert_eq!(settings.number("settings.PAGE_SIZE"), "50");
    assert_eq!(settings.flag("IS_DEBUG"), None);
}

#[test]
fn only_the_configured_user_model_survives_its_swappable_setting() {
    let settings = Settings::default();
    assert_eq!(
        settings.replacement("AUTH_USER_MODEL", &ModelKey::new("auth", "user")),
        None
    );
    assert_eq!(
        settings.replacement("AUTH_USER_MODEL", &ModelKey::new("account", "user")),
        Some(ModelKey::new("auth", "user"))
    );
    assert_eq!(
        settings.replacement("OTHER_MODEL", &ModelKey::new("account", "user")),
        None
    );
}

#[test]
fn a_conditional_user_model_takes_the_branch_without_environment() {
    let sources = vec![
        "if os.environ.get(\"USE_EMAIL\"):
    AUTH_USER_MODEL = \"email.User\"
else:
    AUTH_USER_MODEL = \"custom.User\"
"
        .to_owned(),
    ];
    assert_eq!(
        Settings::read(&sources).user_model,
        ModelKey::new("custom", "User")
    );
}

#[test]
fn falls_back_to_the_django_defaults() {
    let sources = vec!["AUTH_USER_MODEL = get_user_model_name()\n".to_owned()];
    assert_eq!(Settings::read(&sources), Settings::default());
}

#[test]
fn without_settings_files_assumes_the_project_template() {
    let settings = discovered(Path::new("no-existe"));
    assert_eq!(settings.auto_field, Class::BigAuto);
    assert!(!settings.column_drops_cascade);
}

#[test]
fn follows_the_wildcard_imports_of_the_settings_module() {
    let apps_root = std::env::temp_dir().join(format!("kerbo-django-wildcard-{}", std::process::id()));
    let package = apps_root.join("project");
    fs::create_dir_all(&package).expect("se crea el proyecto de prueba");
    fs::write(
        package.join("settings.py"),
        "from .computed import *  # isort: skip\nfrom .missing import *\nfrom .config import DEBUG\n",
    )
    .expect("se escriben los settings");
    fs::write(
        package.join("computed.py"),
        "from .settings import *\nDEFAULT_AUTO_FIELD = \"django.db.models.BigAutoField\"\n",
    )
    .expect("se escribe el módulo importado");
    fs::write(package.join("config.py"), "AUTH_USER_MODEL = \"shop.Customer\"\n")
        .expect("se escribe el módulo que no se importa entero");
    let settings = discovered(&apps_root);
    fs::remove_dir_all(&apps_root).expect("se borra el proyecto de prueba");
    assert_eq!(settings.auto_field, Class::BigAuto);
    assert_eq!(settings.user_model, Settings::default().user_model);
}
