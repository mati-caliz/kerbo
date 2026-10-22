use super::java::JavaSummary;
use super::tests::summary_of;
use super::{IndexedSummary, project_lints};
use crate::jvm::library::Library;

const TAINT_RULES: &[&str] = &["sql-injection", "command-injection", "path-traversal", "xss"];

const USERS: &str = r#"package app;

@RestController
class Users {
    private final JdbcTemplate jdbcTemplate;
    private final UserRepository users;

    List<Object> search(@RequestParam String name) {
        return jdbcTemplate.queryForList("SELECT * FROM users WHERE name = '" + name + "'");
    }

    List<Object> bound(@RequestParam String name) {
        return jdbcTemplate.queryForList("SELECT * FROM users WHERE name = ?", name);
    }

    List<Object> stored(@PathVariable Long id) {
        User user = users.findById(id);
        return jdbcTemplate.queryForList("SELECT * FROM logs WHERE owner = '" + user.getName() + "'");
    }

    void remove(@RequestParam String name) {
        new Helper().delete(name);
    }
}

interface UserRepository {
    User findById(Long id);
}
"#;

const PING: &str = r#"package app;

class Ping extends HttpServlet {
    void doGet(HttpServletRequest request) throws Exception {
        String host = request.getParameter("host");
        Runtime.getRuntime().exec("ping " + new Helper().keep(host));
        Runtime.getRuntime().exec("ping " + new Helper().fixed(host));
        String bar;
        switch ("ABC".charAt(1)) {
            case 'A': bar = host; break;
            default: bar = "safe"; break;
        }
        new java.io.File(bar);
        java.util.List<String> values = new java.util.ArrayList<>();
        values.add("safe");
        values.add(host);
        new java.io.File(values.get(1));
        new java.io.File(values.get(0));
    }
}
"#;

const HELPER: &str = r#"package app;

class Helper {
    String keep(String value) {
        return value.trim();
    }

    String fixed(String value) {
        return "bar";
    }

    void delete(String path) {
        new java.io.File("/data", path).delete();
    }
}
"#;

fn tainted(sources: &[(&str, &str)]) -> Vec<String> {
    let summaries: Vec<JavaSummary> = sources.iter().map(|(_, source)| summary_of(source)).collect();
    let files: Vec<IndexedSummary<'_>> = summaries
        .iter()
        .map(|summary| IndexedSummary {
            summary,
            in_tests: false,
        })
        .collect();
    project_lints(&files, &Library::load(&[]))
        .into_iter()
        .zip(sources)
        .flat_map(|(findings, (name, _))| {
            findings
                .into_iter()
                .filter(|finding| TAINT_RULES.contains(&finding.kind.rule().as_str()))
                .map(move |finding| {
                    format!(
                        "{name}:{} {} {}",
                        finding.line,
                        finding.kind.rule().as_str(),
                        finding.subject
                    )
                })
        })
        .collect()
}

#[test]
fn user_data_reaching_a_query_a_command_or_a_path_is_reported() {
    assert_eq!(
        tainted(&[("Users", USERS), ("Ping", PING), ("Helper", HELPER)]),
        [
            "Users:9 sql-injection queryForList",
            "Ping:6 command-injection exec",
            "Ping:17 path-traversal File",
            "Helper:13 path-traversal File",
        ]
    );
}

const ITEMS: &str = r#"package app;

@Path("/items")
class Items {
    private final DSLContext dsl;

    @GET
    Object find(@QueryParam("q") String name) {
        return dsl.fetch("SELECT * FROM items WHERE name = '" + name + "'");
    }

    Object column(@PathParam("c") String column) {
        return DSL.field(column);
    }

    Object bound(@QueryParam("q") String name) {
        return dsl.fetch("SELECT * FROM items WHERE name = ?", name);
    }
}
"#;

#[test]
fn jax_rs_parameters_reaching_jooq_are_reported() {
    assert_eq!(
        tainted(&[("Items", ITEMS)]),
        ["Items:9 sql-injection fetch", "Items:13 sql-injection field"]
    );
}

#[test]
fn without_a_source_in_the_project_nothing_is_reported() {
    assert_eq!(tainted(&[("Helper", HELPER)]), Vec::<String>::new());
}

const COLLECTIONS: &str = r#"package app;

class Lists extends HttpServlet {
    void doGet(HttpServletRequest request) throws Exception {
        String user = request.getParameter("user");
        java.util.Map<String, String> map = new java.util.HashMap<>();
        map.put("safe", "a");
        map.put("user", user);
        map.put("safe", "b");
        new java.io.File(map.get("safe"));
        new java.io.File(map.get("user"));
        new java.io.File(map.getOrDefault("none", "c"));
        map.remove("user");
        new java.io.File(map.get("user"));
        java.util.List<String> list = new java.util.ArrayList<>();
        list.add("a");
        list.add(0, user);
        list.set(1, "b");
        new java.io.File(list.get(1));
        String first = list.remove(0);
        new java.io.File(first);
        if (list.size() == 1) {
            new java.io.File(list.get(0));
        }
        list.clear();
        list.add(user);
        new java.io.File(list.get(0));
        map.containsKey("x");
        map.clear();
        new java.io.File(map.get("safe"));
    }
}
"#;

#[test]
fn lists_and_maps_are_followed_by_position_and_key() {
    assert_eq!(
        tainted(&[("Lists", COLLECTIONS)]),
        [
            "Lists:11 path-traversal File",
            "Lists:21 path-traversal File",
            "Lists:27 path-traversal File",
        ]
    );
}

const GREET: &str = r#"package app;

class Greet extends HttpServlet {
    void doGet(HttpServletRequest request, HttpServletResponse response) throws Exception {
        String name = request.getParameter("name");
        response.getWriter().print("Hola " + name);
        response.getWriter().print(HtmlUtils.htmlEscape(name));
        java.io.PrintWriter out = response.getWriter();
        out.format("%s", name);
        String escaped = HtmlUtils.htmlEscape(name);
        new java.io.File(escaped);
    }
}
"#;

#[test]
fn an_html_escape_protects_the_response_but_not_a_path() {
    assert_eq!(
        tainted(&[("Greet", GREET)]),
        [
            "Greet:6 xss print",
            "Greet:9 xss format",
            "Greet:11 path-traversal File",
        ]
    );
}

const REPORTS: &str = r#"package app;

@RestController
class Reports {
    private final JdbcTemplate jdbcTemplate;

    void byNames(@RequestParam List<String> names) {
        names.forEach(name -> jdbcTemplate.queryForList("SELECT * FROM r WHERE n = '" + name + "'"));
    }

    void captured(@RequestParam String owner, List<String> tables) {
        tables.forEach(table -> {
            jdbcTemplate.execute("DELETE FROM " + table + " WHERE owner = '" + owner + "'");
        });
    }

    void joined(@RequestParam List<String> ids) {
        String where = ids.stream().map(id -> "id = " + id).collect(Collectors.joining(" OR "));
        jdbcTemplate.execute("DELETE FROM r WHERE " + where);
    }

    void later(@RequestParam String owner) {
        Runnable task = new Runnable() {
            public void run() {
                jdbcTemplate.execute("DELETE FROM r WHERE owner = '" + owner + "'");
            }
        };
        task.run();
    }

    void constant(List<String> tables) {
        tables.forEach(table -> jdbcTemplate.execute("DELETE FROM " + table));
    }
}
"#;

#[test]
fn lambdas_and_anonymous_classes_see_what_they_capture() {
    assert_eq!(
        tainted(&[("Reports", REPORTS)]),
        [
            "Reports:8 sql-injection queryForList",
            "Reports:13 sql-injection execute",
            "Reports:19 sql-injection execute",
            "Reports:25 sql-injection execute",
        ]
    );
}
