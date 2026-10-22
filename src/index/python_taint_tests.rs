use tree_sitter::Parser;

use super::python_taint::{PythonModule, module_name, python_tainted};
use crate::language::Language;
use crate::lint::python::taint::python_taints;
use crate::lint::taint::TaintMethod;

const VIEWS: &str = r#"from flask import request, make_response
import os
import sqlite3
import html
from helpers.wrapper import RequestWrapper

def init(app):
    @app.route('/users')
    def users():
        name = request.args.get("name")
        cur = sqlite3.connect("db").cursor()
        cur.execute(f"SELECT * FROM users WHERE name = '{name}'")
        cur.execute("SELECT * FROM users WHERE name = ?", (name,))
        os.system("ping " + name)
        if '../' in name:
            return "no"
        open(name)
        return html.escape(name)

    @app.route('/echo')
    def echo():
        wrapper = RequestWrapper(request)
        value = wrapper.get_form("x")
        safe = wrapper.get_constant("y")
        open(safe)
        response = make_response(("ok", {"X-Value": value}))
        return value
"#;

const WRAPPER: &str = r#"class RequestWrapper:
    def __init__(self, request):
        self.request = request

    def get_form(self, name):
        return self.request.form.get(name)

    def get_constant(self, name):
        return "bar"
"#;

fn taints_of(source: &str) -> Vec<TaintMethod> {
    let mut parser = Parser::new();
    parser
        .set_language(&Language::Python.grammar().expect("tiene gramática"))
        .expect("la gramática carga");
    let tree = parser.parse(source, None).expect("se parsea");
    python_taints(tree.root_node(), source.as_bytes())
}

#[test]
fn user_data_reaching_python_sinks_is_reported_across_modules() {
    let views = taints_of(VIEWS);
    let wrapper = taints_of(WRAPPER);
    let modules = [
        PythonModule {
            path: "app/views.py",
            taints: &views,
        },
        PythonModule {
            path: "helpers/wrapper.py",
            taints: &wrapper,
        },
    ];
    let found: Vec<String> = python_tainted(&modules)
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
            "12 sql-injection execute",
            "14 command-injection system",
            "27 xss return",
        ]
    );
}

const LAMBDA: &str = r#"import json

def handler(event, context):
    name = event["queryStringParameters"]["name"]
    page = "<p>" + name + "</p>"
    if event.get("json"):
        return {"statusCode": 200, "body": json.dumps({"name": name}), "headers": {"Content-Type": "application/json"}}
    return {"statusCode": 200, "body": page, "headers": {"Content-Type": "text/html"}}
"#;

#[test]
fn a_lambda_response_declared_html_is_a_sink() {
    let handler = taints_of(LAMBDA);
    let modules = [PythonModule {
        path: "handler.py",
        taints: &handler,
    }];
    let found: Vec<String> = python_tainted(&modules)
        .into_iter()
        .flatten()
        .map(|finding| format!("{} {}", finding.line, finding.kind.rule().as_str()))
        .collect();
    assert_eq!(found, ["8 xss"]);
}

#[test]
fn module_names_come_from_the_path() {
    assert_eq!(module_name("helpers/utils.py"), "helpers.utils");
    assert_eq!(module_name("pkg/__init__.py"), "pkg");
    assert_eq!(module_name("./app.py"), "app");
}

const CONSTANTS: &str = r#"from flask import request
import configparser
import os

def init(app):
    @app.route('/benchmark/case')
    def case():
        param = request.form.get("x") or ""
        possible = "ABC"
        guess = possible[1]
        match guess:
            case 'A':
                bar = param
            case 'B' | 'D':
                bar = 'bob'
            case _:
                bar = param
        open(bar)
        num = 106
        other = "fixed" if 7 * 18 + num > 200 else param
        open(other)
        lst = []
        lst.append('safe')
        lst.append(param)
        lst.append('moresafe')
        lst.pop(0)
        open(lst[0])
        open(lst[1])
        values = {'a': 'safe'}
        values['b'] = param
        open(values['a'])
        open(values.get('b'))
        conf = configparser.ConfigParser()
        conf.set('section', 'keyA', 'safe')
        conf.set('section', 'keyB', param)
        open(conf.get('section', 'keyA'))
        open(conf.get('section', 'keyB'))
        if 'should' not in "This should never happen":
            open(param)
        elif num > 1000:
            open(param)
        text = ""
        for item in request.args.getlist("y"):
            text += item
            if not item:
                break
        while False:
            text = "never"
        try:
            with open("a") as handle:
                data = handle.read()
        except IOError:
            data = ""
        finally:
            pass
        os.popen(text)
        parts = request.path.split("/")
        open(parts[1])
        return ""
"#;

#[test]
fn constants_branches_and_collections_decide_what_reaches_a_sink() {
    let views = taints_of(CONSTANTS);
    let modules = [PythonModule {
        path: "views.py",
        taints: &views,
    }];
    let found: Vec<String> = python_tainted(&modules)
        .into_iter()
        .flatten()
        .map(|finding| format!("{} {}", finding.line, finding.kind.rule().as_str()))
        .collect();
    assert_eq!(
        found,
        [
            "27 path-traversal",
            "32 path-traversal",
            "37 path-traversal",
            "56 command-injection",
        ]
    );
}
