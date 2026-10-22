use tree_sitter::Parser;

use super::ecmascript_taint::{ScriptModule, ecmascript_tainted, module_name};
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

fn taints_of(source: &str) -> Vec<TaintMethod> {
    let mut parser = Parser::new();
    parser
        .set_language(&Language::JavaScript.grammar().expect("tiene gramática"))
        .expect("la gramática carga");
    let tree = parser.parse(source, None).expect("se parsea");
    ecmascript_taints(tree.root_node(), source.as_bytes())
}

fn found(modules: &[ScriptModule<'_>]) -> Vec<String> {
    ecmascript_tainted(modules)
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
            "45 xss return",
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
fn module_names_drop_extensions_and_index() {
    assert_eq!(module_name("src/helpers.js"), "src/helpers");
    assert_eq!(module_name("src/lib/index.ts"), "src/lib");
}
