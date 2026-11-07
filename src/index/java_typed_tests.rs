use super::java_nullness::Resolved;
use super::java_typed::{Typed, library_field, library_parents, library_returned};
use crate::jvm::class_file::{ClassField, ClassFile};
use crate::jvm::method::Method;

const LIST: &str = "java.util.List";
const FAILURE: &str = "app.Failure";

fn library(name: &str) -> Resolved {
    Resolved::Library(name.to_owned())
}

fn method(name: &str, descriptor: &str, signature: &str) -> Method {
    Method {
        name: name.to_owned(),
        descriptor: descriptor.to_owned(),
        access: 0,
        parameter_annotations: Vec::new(),
        return_annotations: Vec::new(),
        exceptions: Vec::new(),
        signature: Some(signature.to_owned()),
    }
}

fn array_list() -> ClassFile {
    ClassFile {
        name: "java.util.ArrayList".to_owned(),
        superclass: Some("java.lang.Object".to_owned()),
        interfaces: vec![LIST.to_owned()],
        annotations: Vec::new(),
        methods: vec![
            method("get", "(I)Ljava/lang/Object;", "(I)TE;"),
            method(
                "toArray",
                "([Ljava/lang/Object;)[Ljava/lang/Object;",
                "<T:Ljava/lang/Object;>([TT;)[TT;",
            ),
            method("first", "()Ljava/lang/Object;", "<E:Ljava/lang/Object;>()TE;"),
        ],
        fields: vec![ClassField {
            name: "EMPTY".to_owned(),
            descriptor: "Ljava/util/List;".to_owned(),
            signature: Some("Ljava/util/List<Ljava/lang/String;>;".to_owned()),
        }],
        signature: Some("<E:Ljava/lang/Object;>Ljava/lang/Object;Ljava/util/List<TE;>;".to_owned()),
    }
}

#[test]
fn a_type_parameter_takes_the_argument_of_the_receiver_and_passes_it_to_the_parents() {
    let file = array_list();
    let failures = Typed {
        class: library("java.util.ArrayList"),
        arguments: vec![Some(Typed::raw(library(FAILURE)))],
    };
    let get = file.methods.first().expect("get está");
    assert_eq!(
        library_returned(&file, get, &failures),
        Some(Typed::raw(library(FAILURE)))
    );
    assert_eq!(
        library_returned(&file, get, &Typed::raw(library("java.util.ArrayList"))),
        Some(Typed::raw(library("java.lang.Object")))
    );
    let shadowed = file.methods.get(2).expect("first está");
    assert_eq!(
        library_returned(&file, shadowed, &failures),
        Some(Typed::raw(library("java.lang.Object")))
    );
    assert_eq!(
        library_parents(&file, &failures),
        [
            Typed::raw(library("java.lang.Object")),
            Typed {
                class: library(LIST),
                arguments: vec![Some(Typed::raw(library(FAILURE)))],
            },
        ]
    );
    assert_eq!(
        library_field(&file, "EMPTY"),
        Some(Typed {
            class: library(LIST),
            arguments: vec![Some(Typed::raw(library("java.lang.String")))],
        })
    );
    assert_eq!(library_field(&file, "MISSING"), None);
}
