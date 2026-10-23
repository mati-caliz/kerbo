use tree_sitter::Parser;

use super::ecmascript_taint::{ScriptModule, ecmascript_tainted, module_name};
use super::templates::Templates;
use crate::language::Language;
use crate::lint::ecmascript::taint::ecmascript_taints;
use crate::lint::taint::TaintMethod;

const ROUTES: &str = r"const express = require('express');
const { exec, spawn } = require('child_process');
const fs = require('node:fs');
const path = require('path');
const { lookup } = require('./helpers');
const app = express();

app.get('/users', (req, res) => {
  const name = req.query.name;
  db.query(`SELECT * FROM users WHERE name = '${name}'`);
  db.query('SELECT * FROM users WHERE name = ?', [name]);
  exec('ping ' + name);
  spawn('ping', [name]);
  spawn('ping', [name], { shell: true });
  fs.readFile(path.basename(name), () => {});
  fs.readFileSync(lookup(name));
  res.send('<p>' + name + '</p>');
});

app.get('/json', (req, res) => {
  res.type('json');
  res.send(req.query.name);
});

app.get('/raw', (request, response) => {
  response.writeHead(200, { 'content-type': 'application/json' });
  response.end(request.query.name);
});

app.get('/fastify', async (request, reply) => {
  return reply.send(request.query.name);
});

exports.handler = async (event) => {
  const connection = knex({ client: 'pg' });
  await connection.raw(`SELECT * FROM items WHERE id = ${event.pathParameters.id}`);
};

const { Hono } = require('hono');
const hono = new Hono();
hono.get('/hono', (c) => {
  db.query('SELECT * FROM t WHERE id = ' + c.req.query('id'));
});

export default async function Page({ searchParams }) {
  db.query(`SELECT * FROM t WHERE q = '${searchParams.q}'`);
}

exports.page = function (event, context) {
  const html = `<div>${event.name}</div>`;
  const json = { statusCode: 200, body: html, headers: { 'Content-Type': 'application/json' } };
  if (event.json) {
    return json;
  }
  return { statusCode: 200, body: html, headers: { 'Content-Type': 'text/html' } };
};
";

const HELPERS: &str = r"function lookup(name) {
  return '/data/' + name;
}

module.exports = { lookup };
";

const BROWSER: &str = r"function render() {
  const fragment = location.hash;
  document.write(fragment);
  const items = [];
  items.push('safe');
  items.push(fragment);
  document.getElementById('a').innerHTML = items.shift();
  document.getElementById('b').innerHTML = items.shift();
  const mode = 'b';
  switch (mode) {
    case 'a':
      document.body.innerHTML = fragment;
      break;
    default:
      document.body.innerHTML = 'fixed';
  }
  jQuery('<span>' + fragment + '</span>');
  $('#title').html(fragment);
  return <div dangerouslySetInnerHTML={{ __html: fragment }} />;
}
";

const NEST: &str = r"@Controller('items')
export class ItemsController {
  constructor(private readonly db: Pool) {}

  @Get()
  find(@Query('q') name: string, limit: number) {
    return this.db.query(`SELECT * FROM items WHERE name = '${name}' LIMIT ${limit}`);
  }
}
";

const CLOSURES: &str = r"function register(app) {
  app.get('/search', (req, res) => {
    const term = req.query.term;
    db.query('SELECT 1').then(() => {
      db.query(`SELECT * FROM logs WHERE term = '${term}'`);
    });
    const names = [term].map((name) => '<b>' + name + '</b>');
    let html = '';
    ['a', 'b'].forEach((term) => {
      html += term;
    });
    res.send(names.join(''));
    res.send(html + term);
  });
  app.get('/plain', (req, res) => {
    res.type('json');
    res.send(req.query.term);
  });
  app.get('/after', (req, res) => {
    let shown = '';
    [req.query.term].forEach((item) => {
      shown += item;
    });
    res.send(shown);
  });
}
";

fn taints_of(source: &str) -> Vec<TaintMethod> {
    taints_in(Language::JavaScript, source)
}

fn taints_in(language: Language, source: &str) -> Vec<TaintMethod> {
    let mut parser = Parser::new();
    parser
        .set_language(&language.grammar().expect("tiene gramática"))
        .expect("la gramática carga");
    let tree = parser.parse(source, None).expect("se parsea");
    ecmascript_taints(tree.root_node(), source.as_bytes())
}

fn found(modules: &[ScriptModule<'_>]) -> Vec<String> {
    found_with(modules, &Templates::default())
}

fn found_with(modules: &[ScriptModule<'_>], templates: &Templates) -> Vec<String> {
    ecmascript_tainted(modules, templates)
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
        .collect()
}

#[test]
fn user_data_reaching_node_sinks_is_reported_across_modules() {
    let routes = taints_of(ROUTES);
    let helpers = taints_of(HELPERS);
    let modules = [
        ScriptModule {
            path: "src/routes.js",
            taints: &routes,
        },
        ScriptModule {
            path: "src/helpers.js",
            taints: &helpers,
        },
    ];
    assert_eq!(
        found(&modules),
        [
            "10 sql-injection query",
            "12 command-injection exec",
            "14 command-injection spawn",
            "16 path-traversal readFileSync",
            "17 xss send",
            "36 sql-injection raw",
            "42 sql-injection query",
            "46 sql-injection query",
            "55 xss return",
        ]
    );
}

#[test]
fn browser_sources_reach_the_document() {
    let browser = taints_of(BROWSER);
    let modules = [ScriptModule {
        path: "src/render.jsx",
        taints: &browser,
    }];
    assert_eq!(
        found(&modules),
        [
            "3 xss write",
            "8 xss document.getElementById('b').innerHTML",
            "17 xss jQuery",
            "18 xss html",
            "19 xss dangerouslySetInnerHTML",
        ]
    );
}

#[test]
fn nest_decorated_parameters_come_from_the_user() {
    let controller = taints_in(Language::TypeScript, NEST);
    let modules = [ScriptModule {
        path: "src/items.controller.ts",
        taints: &controller,
    }];
    assert_eq!(found(&modules), ["7 sql-injection query"]);
}

const VIEWS: &str = r"app.get('/hello', (req, res) => {
  res.render('hello', { name: req.query.name, title: 'fijo' });
});
";

#[test]
fn express_templates_decide_what_is_raw() {
    let views = taints_of(VIEWS);
    let modules = [ScriptModule {
        path: "src/views.js",
        taints: &views,
    }];
    let templates = Templates::of(vec![(
        "views/hello.ejs".to_owned(),
        "<h1><%- title %></h1><p><%- name %></p>".to_owned(),
    )]);
    assert_eq!(found_with(&modules, &templates), ["2 xss hello"]);
}

#[test]
fn callbacks_see_what_the_enclosing_function_sees() {
    let routes = taints_of(CLOSURES);
    let modules = [ScriptModule {
        path: "src/routes.js",
        taints: &routes,
    }];
    assert_eq!(
        found(&modules),
        [
            "5 sql-injection query",
            "12 xss send",
            "13 xss send",
            "24 xss send",
        ]
    );
}

const SCRIPT: &str = r"const fragment = location.hash;
document.write(fragment);
export const title = 'fijo';
document.title = title;
if (fragment) {
  document.body.innerHTML = '<p>' + fragment + '</p>';
}
";

#[test]
fn code_outside_functions_is_followed_too() {
    let script = taints_of(SCRIPT);
    let modules = [ScriptModule {
        path: "src/page.js",
        taints: &script,
    }];
    assert_eq!(found(&modules), ["2 xss write", "6 xss document.body.innerHTML"]);
}

#[test]
fn module_names_drop_extensions_and_index() {
    assert_eq!(module_name("src/helpers.js"), "src/helpers");
    assert_eq!(module_name("src/lib/index.ts"), "src/lib");
}
