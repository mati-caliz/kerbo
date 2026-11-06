use std::path::{Path, PathBuf};

use super::class_file::{ClassFile, parameter_count, parse};
use super::jdk::Jdk;
use super::jimage::ModulesImage;
use super::method::Method;
use super::release::project_release;
use super::zip::Archive;

const COMPRESSED: &str = "sample.jar";
const STORED: &str = "stored.jar";
const FILTER: &str = "sample/web/Filter.class";
const PACKAGE_INFO: &str = "sample/web/package-info.class";

#[test]
fn reads_compressed_and_stored_entries_by_name() {
    for jar in [COMPRESSED, STORED] {
        let archive = Archive::open(&fixture(jar)).expect("el jar se abre");
        assert!(archive.names().any(|name| name == FILTER));
        let filter = archive.read(FILTER).expect("la entrada está");
        assert_eq!(filter.get(..4), Some(&[0xCA, 0xFE, 0xBA, 0xBE][..]));
        assert!(archive.read("no/existe.class").is_none());
    }
    assert!(Archive::open(&Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml")).is_err());
}

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("src/jvm/fixtures")
        .join(name)
}

#[test]
fn a_class_file_gives_its_parents_methods_and_parameter_annotations() {
    let archive = Archive::open(&fixture(COMPRESSED)).expect("el jar se abre");
    let filter = parse(&archive.read(FILTER).expect("la clase está")).expect("la clase se lee");
    assert_eq!(filter.name, "sample.web.Filter");
    assert_eq!(filter.superclass.as_deref(), Some("java.lang.Object"));
    assert_eq!(filter.interfaces, ["java.lang.Runnable", "java.lang.Comparable"]);
    let handle = filter
        .methods
        .iter()
        .find(|method| method.name == "handle")
        .expect("handle está");
    assert_eq!(handle.parameter_count(), 3);
    assert_eq!(
        handle.parameter_annotations,
        [
            Vec::<String>::new(),
            vec!["sample.annotations.Nullable".to_owned()],
            vec!["sample.annotations.Marker".to_owned()],
        ]
    );
    let hidden = filter
        .methods
        .iter()
        .find(|method| method.name == "hidden")
        .expect("hidden está");
    assert!(hidden.is_private() && !hidden.is_static());
    assert_eq!(hidden.parameter_count(), 2);
    assert!(
        filter
            .methods
            .iter()
            .any(|method| method.name == "describe" && method.is_static())
    );
    let package = parse(&archive.read(PACKAGE_INFO).expect("el package-info está")).expect("se lee");
    assert_eq!(package.annotations, ["sample.annotations.NullMarked"]);
    assert!(parse(b"\xCA\xFE\xBA\xBE").is_err());
}

#[test]
fn descriptors_count_their_parameters() {
    assert_eq!(parameter_count("()V"), 0);
    assert_eq!(parameter_count("(ILjava/lang/String;[J[[Ljava/util/List;D)V"), 5);
    assert_eq!(parameter_count("sin paréntesis"), 0);
}

#[test]
fn the_classpath_comes_from_the_poms_and_survives_their_changes() {
    use super::classpath::{ask_maven, fingerprint, jars_in, pom_files};
    use crate::test_support::TempTree;
    let tree = TempTree::new("jvm-classpath");
    tree.write("pom.xml", "<project/>");
    tree.write("module/pom.xml", "<project/>");
    tree.write("target/pom.xml", "<project/>");
    let poms = pom_files(tree.path());
    assert_eq!(poms.len(), 2, "target no cuenta: {poms:?}");
    let before = fingerprint(tree.path(), &poms);
    tree.write("module/pom.xml", "<project><version>2</version></project>");
    assert_ne!(fingerprint(tree.path(), &poms), before);
    let separator = if cfg!(windows) { ";" } else { ":" };
    let listed = format!("/m2/a.jar{separator}/m2/b.jar\r\n/m2/a.jar\n\n");
    assert_eq!(
        jars_in(&listed),
        [PathBuf::from("/m2/a.jar"), PathBuf::from("/m2/b.jar")]
    );
    let saved = tree.path().join("cache/classpath.txt");
    assert_eq!(
        ask_maven(tree.path(), &saved).filter(|found| !found.is_empty()),
        None
    );
}

#[test]
fn gradle_projects_are_found_by_their_build_files_and_asked_through_the_wrapper() {
    use super::classpath::files_named;
    use super::gradle::{BUILD_FILES, ask_gradle};
    use crate::test_support::TempTree;
    let tree = TempTree::new("jvm-gradle");
    tree.write("settings.gradle.kts", "rootProject.name = \"app\"");
    tree.write("api/build.gradle", "plugins { id 'java' }");
    tree.write("build/build.gradle", "");
    let found = files_named(tree.path(), BUILD_FILES);
    assert_eq!(found.len(), 2, "build no cuenta: {found:?}");
    let saved = tree.path().join("cache/classpath.txt");
    assert_eq!(
        ask_gradle(tree.path(), &saved).filter(|listed| !listed.is_empty()),
        None
    );
    if cfg!(not(windows)) {
        tree.write(
            "gradlew",
            "for argument in \"$@\"; do case \"$argument\" in -Dkerbo.classpath.output=*) printf '/g/a.jar\n' > \"${argument#*=}\";; esac; done
",
        );
        assert_eq!(
            ask_gradle(tree.path(), &saved).as_deref(),
            Some(
                "/g/a.jar
"
            )
        );
        tree.write(
            "gradlew", "exit 1
",
        );
        assert_eq!(ask_gradle(tree.path(), &saved), None);
    }
}

#[test]
fn a_varargs_method_accepts_more_arguments_and_knows_its_class_parameters() {
    let method = Method {
        name: "queryForObject".to_owned(),
        descriptor: "(Ljava/lang/String;Ljava/lang/Class;[Ljava/lang/Object;)Ljava/lang/Object;".to_owned(),
        access: 0x0080,
        parameter_annotations: vec![Vec::new(); 3],
        return_annotations: Vec::new(),
        exceptions: Vec::new(),
    };
    assert!(method.accepts(3) && method.accepts(2) && method.accepts(6));
    assert!(!method.accepts(1));
    assert!(method.takes_class_at(1) && !method.takes_class_at(0) && !method.takes_class_at(2));
    assert_eq!(method.return_class().as_deref(), Some("java.lang.Object"));
    assert!(method.returns_reference());
    assert!(!method.is_private() && !method.is_static());
    let fixed = Method {
        descriptor: "(IJ)V".to_owned(),
        access: 0,
        parameter_annotations: vec![Vec::new(); 2],
        ..method
    };
    assert!(fixed.accepts(2) && !fixed.accepts(3));
    assert!(!fixed.returns_reference() && fixed.return_class().is_none());
}

const JAVA_17: u32 = 17;
const JAVA_8: u32 = 8;
const FILE_INPUT_STREAM: &str = "java/io/FileInputStream.class";

fn parsed(jdk: &Jdk, entry: &str) -> ClassFile {
    parse(&jdk.read(entry).expect("la firma está")).expect("la firma se lee")
}

#[test]
fn the_jdk_signatures_come_from_ct_sym_for_an_older_release_with_their_exceptions() {
    let jdk = Jdk::at(&fixture("jdk"), Some(JAVA_17)).expect("el JDK se abre");
    assert!(jdk.contains(FILE_INPUT_STREAM) && !jdk.contains("no/Existe.class"));
    let stream = parsed(&jdk, FILE_INPUT_STREAM);
    let close = stream
        .methods
        .iter()
        .find(|method| method.name == "close")
        .expect("close está");
    assert_eq!(close.exceptions, ["java.io.IOException"]);
    let old = Jdk::at(&fixture("jdk"), Some(JAVA_8)).expect("el JDK se abre");
    assert!(old.contains(FILE_INPUT_STREAM));
    assert!(!old.contains("java/lang/constant/Constable.class"));
    assert!(jdk.contains("java/lang/constant/Constable.class"));
}

// `lib/modules` con una clase sin comprimir, una comprimida y un recurso que no es una clase.
fn modules_image() -> Vec<u8> {
    let strings = b"\0java/lang\0Hola\0class\0Comprimida\0txt\0";
    let (parent, hola, class, compressed, text) = (1, 11, 16, 22, 33);
    let class_bytes = b"\xCA\xFE\xBA\xBE";
    let location = |base: u8, extension: u8, packed: u8| {
        vec![
            0b0001_0000,
            parent,
            0b0001_1000,
            base,
            0b0010_0000,
            extension,
            0b0010_1000,
            0,
            0b0011_0000,
            packed,
            0b0011_1000,
            4,
            0,
        ]
    };
    let locations = [
        location(hola, class, 0),
        location(compressed, class, 4),
        location(hola, text, 0),
    ];
    let offsets: Vec<u32> = vec![0, 13, 26];
    let locations_size: u32 = 39;
    let mut image = Vec::new();
    let header = [0xCAFE_DADA, 0x0001_0000, 0, 3, 3, locations_size, 37];
    image.extend(header.iter().flat_map(|slot: &u32| slot.to_le_bytes()));
    image.extend([0_u8; 12]);
    image.extend(offsets.iter().flat_map(|offset| offset.to_le_bytes()));
    image.extend(locations.iter().flatten());
    image.extend(strings);
    image.extend(class_bytes);
    image
}

#[test]
fn the_modules_image_lists_uncompressed_classes_and_reads_them() {
    use crate::test_support::TempTree;
    let tree = TempTree::new("jvm-jimage");
    let path = tree.path().join("modules");
    std::fs::write(&path, modules_image()).expect("se escribe");
    let image = ModulesImage::open(&path).expect("la imagen se abre");
    assert!(image.contains("java/lang/Hola.class"));
    assert!(!image.contains("java/lang/Comprimida.class"));
    assert_eq!(
        image.read("java/lang/Hola.class").as_deref(),
        Some(&b"\xCA\xFE\xBA\xBE"[..])
    );
    assert!(image.read("no/Existe.class").is_none());
    std::fs::write(&path, b"no es una imagen de jimage, ni de lejos").expect("se escribe");
    assert!(ModulesImage::open(&path).is_none());
}

#[test]
fn the_release_comes_from_the_build_and_skips_what_is_not_a_number() {
    use crate::test_support::TempTree;
    let maven = TempTree::new("jvm-release-maven");
    maven.write(
        "pom.xml",
        "<project><maven.compiler.release>${java.version}</maven.compiler.release><java.version>17</java.version></project>",
    );
    assert_eq!(project_release(maven.path()), Some(17));
    let legacy = TempTree::new("jvm-release-legacy");
    legacy.write(
        "pom.xml",
        "<project><maven.compiler.source>1.8</maven.compiler.source></project>",
    );
    assert_eq!(project_release(legacy.path()), Some(8));
    let gradle = TempTree::new("jvm-release-gradle");
    gradle.write(
        "build.gradle",
        "java { sourceCompatibility = JavaVersion.VERSION_11 }",
    );
    assert_eq!(project_release(gradle.path()), Some(11));
    assert_eq!(project_release(TempTree::new("jvm-release-none").path()), None);
}
