use tree_sitter::Parser;

use super::go_taint::{GoFile, go_tainted, package_of};
use crate::language::Language;
use crate::lint::go::taint::go_taints;
use crate::lint::taint::TaintMethod;

const HANDLERS: &str = r#"package handlers

import (
	"database/sql"
	"fmt"
	"html"
	"net/http"
	"os"
	"os/exec"
	"path"
	"path/filepath"
	"strconv"
	"text/template"

	"github.com/gin-gonic/gin"
	"github.com/me/app/internal/store"
	"gorm.io/gorm"
)

func Search(db *sql.DB) func(w http.ResponseWriter, r *http.Request) {
	return func(w http.ResponseWriter, r *http.Request) {
		name := r.URL.Query().Get("name")
		db.Query("SELECT * FROM users WHERE name = '" + name + "'")
		db.QueryContext(r.Context(), "SELECT * FROM users WHERE name = ?", name)
		id, _ := strconv.Atoi(name)
		db.Exec(fmt.Sprintf("DELETE FROM users WHERE id = %d", id))
		exec.Command("sh", "-c", "ping "+name).Run()
		exec.Command("ping", name).Run()
		exec.Command(name, "--version").Run()
		os.ReadFile(filepath.Base(name))
		os.ReadFile(path.Clean("/" + name))
		os.Open(store.Resolve(name))
		fmt.Fprintf(w, "<p>%s</p>", name)
		w.Write([]byte(html.EscapeString(name)))
	}
}

func Orders(w http.ResponseWriter, r *http.Request, db *gorm.DB) {
	order := r.FormValue("order")
	table := db.Table("orders")
	table.Order(order).Find(nil)
	table.Order("created_at").Find(nil)
	w.Header().Set("Content-Type", "application/json")
	w.Write([]byte(order))
}

func Page(c *gin.Context) {
	name := c.Query("name")
	c.Data(200, "text/html; charset=utf-8", []byte("<p>"+name+"</p>"))
	c.Data(200, "application/json", []byte(name))
}

func Render(w http.ResponseWriter, r *http.Request) {
	page := template.Must(template.New("page").Parse("<p>{{.}}</p>"))
	page.Execute(w, r.FormValue("name"))
}

func Mode(w http.ResponseWriter, r *http.Request) {
	mode := "b"
	values := map[string]string{"safe": "fixed"}
	values["user"] = r.FormValue("x")
	switch mode {
	case "a":
		w.Write([]byte(values["user"]))
	default:
		w.Write([]byte(values["safe"]))
	}
	for _, item := range r.URL.Query()["items"] {
		fmt.Fprintln(w, item)
	}
}
"#;

const STORE: &str = r#"package store

func Resolve(name string) string {
	return "/data/" + name
}
"#;

const LAMBDA: &str = r#"package main

import "github.com/aws/aws-lambda-go/events"

func handler(request events.APIGatewayProxyRequest) (events.APIGatewayProxyResponse, error) {
	page := "<div>" + request.Body + "</div>"
	return events.APIGatewayProxyResponse{
		StatusCode: 200,
		Body:       page,
		Headers:    map[string]string{"Content-Type": "text/html"},
	}, nil
}
"#;

fn taints_of(source: &str) -> Vec<TaintMethod> {
    let mut parser = Parser::new();
    parser
        .set_language(&Language::Go.grammar().expect("tiene gramática"))
        .expect("la gramática carga");
    let tree = parser.parse(source, None).expect("se parsea");
    go_taints(tree.root_node(), source.as_bytes())
}

#[test]
fn user_data_reaching_go_sinks_is_reported_across_packages() {
    let handlers = taints_of(HANDLERS);
    let store = taints_of(STORE);
    let lambda = taints_of(LAMBDA);
    let files = [
        GoFile {
            path: "internal/handlers/handlers.go",
            taints: &handlers,
        },
        GoFile {
            path: "internal/store/store.go",
            taints: &store,
        },
        GoFile {
            path: "cmd/lambda/main.go",
            taints: &lambda,
        },
    ];
    let found: Vec<String> = go_tainted(&files)
        .into_iter()
        .flatten()
        .map(|finding| {
            format!(
                "{} {} {}",
                finding.line,
                finding.kind.rule().as_str(),
                finding.subject
            )
        })
        .collect();
    assert_eq!(
        found,
        [
            "23 sql-injection Query",
            "27 command-injection Command",
            "29 command-injection Command",
            "32 path-traversal Open",
            "33 xss Fprintf",
            "41 sql-injection Order",
            "49 xss Data",
            "55 xss Execute",
            "69 xss Fprintln",
            "7 xss return",
        ]
    );
}

#[test]
fn the_package_is_the_directory() {
    assert_eq!(package_of("internal/store/store.go"), "internal/store");
    assert_eq!(package_of("./main.go"), "");
}
