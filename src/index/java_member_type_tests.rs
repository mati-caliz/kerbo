use std::path::Path;

use super::java_null_tests::findings_in;
use super::java_throws_tests::fixture_jdk;
use super::tests::summary_of;
use super::{IndexedSummary, project_lints};
use crate::jvm::library::Library;

const MEMBER_ROOTS: &str = r"package app;

import org.jspecify.annotations.Nullable;
import sample.fields.Rule;

public class Jenkins {
    @Nullable View getView(String name) {
        return null;
    }
}

final class View {
    int size() {
        return 0;
    }
}

final class JenkinsRule {
    Jenkins jenkins;
}

final class ViewTest {
    int count(JenkinsRule rule) {
        return rule.jenkins.getView(rule.toString()).size();
    }

    int packaged(Rule rule) {
        return rule.jenkins.getView(rule.toString()).size();
    }

    int unknown(Rule rule) {
        return rule.missing.getView(rule.toString()).size();
    }
}
";

// Una cadena que empieza en el campo de una variable (`j.jenkins.getView(...)`) toma el tipo del
// campo. Sin la librería, el campo de `Rule` no se conoce y la cadena no sigue.
#[test]
fn a_chain_rooted_at_a_field_of_a_variable_uses_the_field_type() {
    assert_eq!(findings_in(MEMBER_ROOTS, "data-flow-issue"), ["24:54 size()"]);
}

// El campo de una clase de un jar sale de su `.class`: `sample.fields.Rule` declara
// `public app.Jenkins jenkins`.
#[test]
fn the_field_of_a_library_class_comes_from_its_class_file() {
    let jar = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/jvm/fixtures/fields.jar");
    let summary = summary_of(MEMBER_ROOTS);
    let files = [IndexedSummary {
        summary: &summary,
        in_tests: false,
    }];
    let library = Library::load(&[jar]).with_jdk(fixture_jdk());
    let found: Vec<String> = project_lints(&files, &library)
        .into_iter()
        .flatten()
        .filter(|finding| finding.kind.rule().as_str() == "data-flow-issue")
        .map(|finding| format!("{}:{} {}", finding.line, finding.column, finding.subject))
        .collect();
    assert_eq!(found, ["24:54 size()", "28:54 size()"]);
}

const INHERITED_ROOTS: &str = r"package app;

import org.jspecify.annotations.Nullable;

public class Jenkins {
    PluginManager pluginManager;

    @Nullable View getView(String name) {
        return null;
    }

    @Nullable Object getItem(String name) {
        return null;
    }
}

final class PluginManager {
    @Nullable View getPlugin(String name) {
        return null;
    }
}

final class View {
    int size() {
        return 0;
    }
}

final class JenkinsRule {
    Jenkins jenkins;
}

class TestBase {
    protected JenkinsRule j;
    protected Jenkins direct;
}

final class ViewTest extends TestBase {
    int inherited() {
        return j.jenkins.getView(j.toString()).size();
    }

    int inheritedDirectly() {
        return direct.getView(direct.toString()).size();
    }

    int throughTwoFields(JenkinsRule rule) {
        return rule.jenkins.pluginManager.getPlugin(rule.toString()).size();
    }

    int fromAnonymous() {
        return new Object() {
            int count() {
                return j.jenkins.getView(j.toString()).size();
            }
        }.count();
    }

    int cast() {
        return ((View) j.jenkins.getItem(j.toString())).size();
    }

    int undeclared() {
        return missing.getView(j.toString()).size();
    }
}
";

// `j` lo declara la clase de arriba: la cadena toma el tipo del campo heredado, también desde una
// clase anónima, y sigue por varios campos (`rule.jenkins.pluginManager`). El casteo de una llamada
// que puede dar null no la salva.
#[test]
fn a_chain_rooted_at_an_inherited_field_uses_the_field_type() {
    assert_eq!(
        findings_in(INHERITED_ROOTS, "data-flow-issue"),
        [
            "40:48 size()",
            "44:50 size()",
            "48:70 size()",
            "54:56 size()",
            "60:57 getItem()"
        ]
    );
}

const CLASS_OVERLOADS: &str = r"package app;

import org.jspecify.annotations.Nullable;

public class ExtensionList {
    @Nullable Monitor get(Class<Monitor> type) {
        return null;
    }

    Monitor get(int index) {
        return new Monitor();
    }

    static ExtensionList lookup() {
        return new ExtensionList();
    }
}

final class Monitor {
    boolean active;

    boolean isActivated() {
        return active;
    }

    static boolean byClass() {
        return ExtensionList.lookup().get(Monitor.class).isActivated();
    }

    static boolean byIndex() {
        return ExtensionList.lookup().get(0).isActivated();
    }

    static boolean fromVariable() {
        Monitor monitor = ExtensionList.lookup().get(Monitor.class);
        return monitor.isActivated();
    }
}
";

// Con un literal de clase como argumento, la sobrecarga es la que declara `Class`, no `get(int)`.
#[test]
fn a_class_literal_picks_the_overload_that_declares_a_class_parameter() {
    assert_eq!(
        findings_in(CLASS_OVERLOADS, "data-flow-issue"),
        ["27:58 isActivated()", "36:24 isActivated()"]
    );
}

const INHERITED_NULLNESS: &str = r"package app;

import java.io.File;
import org.jspecify.annotations.Nullable;

interface ItemGroup {
    @Nullable Item getItem(String name);

    @Nullable Item getParent();
}

class Item {
    void delete() {
    }

    Item getParent() {
        return this;
    }
}

class Folder implements ItemGroup {
    @Override
    public Item getItem(String name) {
        return new Item();
    }

    @Override
    public Item getParent() {
        return new Item();
    }

    static void overridden(Folder folder) {
        folder.getItem(folder.toString()).delete();
    }

    static void assignedInside(Folder folder) {
        Item item = new Item();
        (item = folder.getItem(folder.toString())).delete();
        item.delete();
    }

    static int iterated(File folder) {
        int count = 0;
        for (File child : folder.listFiles()) {
            count += child.hashCode();
        }
        File[] children = folder.listFiles();
        for (File child : children) {
            count += child.hashCode();
        }
        return count;
    }
}
";

// Un método sin anotación hereda la `@Nullable` del que redefine; una asignación entre paréntesis
// usada como receptor desreferencia la variable recién asignada, y un `for` desreferencia lo que
// recorre.
#[test]
fn inherited_annotations_parenthesized_assignments_and_iterated_values_are_dereferenced() {
    assert_eq!(
        findings_in(INHERITED_NULLNESS, "data-flow-issue"),
        [
            "33:43 delete()",
            "38:52 delete()",
            "44:27 listFiles()",
            "48:27 children()"
        ]
    );
}

const ABSENT_ALTERNATIVES: &str = r#"package app;

import java.util.Optional;

final class Element {
    Optional<Element> child(String name) {
        return Optional.empty();
    }

    String text() {
        return "";
    }

    static String chained(Element root) {
        return root.child("a").orElse(null).text();
    }

    static String assigned(Element root) {
        Element found = root.child("a").orElse(null);
        return found.text();
    }

    static String unresolved(Unknown root) {
        return root.child("a").orElse(null).text();
    }

    static String withDefault(Element root) {
        return root.child("a").orElse(root).text();
    }
}
"#;

// `orElse(null)` puede dar null aunque no se sepa el tipo de lo que está antes; con otro valor, no.
#[test]
fn an_optional_resolved_with_a_null_alternative_may_be_null() {
    assert_eq!(
        findings_in(ABSENT_ALTERNATIVES, "data-flow-issue"),
        ["15:45 text()", "20:22 text()", "24:45 text()"]
    );
}

const INHERITED_QUEUE_METHODS: &str = r"package app;

import java.util.ArrayDeque;
import java.util.Deque;

final class Pending {
    static int first(Deque<String> names) {
        return names.peek().length();
    }

    static int last(ArrayDeque<String> names) {
        return names.pollLast().length();
    }

    static int guarded(Deque<String> names) {
        while (!names.isEmpty()) {
            return names.poll().length();
        }
        return 0;
    }

    static int assigned(ArrayDeque<String> names) {
        if (!names.isEmpty()) {
            String name = names.peek();
            return name.length();
        }
        String name = names.peek();
        return name.length();
    }
}
";

// `Deque` hereda `peek()` de `Queue`: lo que el JDK da por posiblemente null vale para sus hijos,
// salvo adentro de un `if` o un `while` que da la cola por no vacía, también si pasa por una variable.
#[test]
fn queue_methods_inherited_by_a_deque_may_return_null() {
    assert_eq!(
        findings_in(INHERITED_QUEUE_METHODS, "data-flow-issue"),
        ["8:29 length()", "12:33 length()", "28:21 length()"]
    );
}
