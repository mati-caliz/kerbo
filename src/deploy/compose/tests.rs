use tree_sitter::Parser;

use super::violations;
use crate::language::Language;
use crate::rules::{ComposeRule, RuleId};

fn found(source: &str) -> Vec<(ComposeRule, usize, String)> {
    let mut parser = Parser::new();
    parser
        .set_language(&Language::Compose.grammar().expect("tiene gramática"))
        .expect("la gramática carga");
    let tree = parser.parse(source, None).expect("se parsea");
    violations(tree.root_node(), source.as_bytes())
        .into_iter()
        .map(|violation| {
            let RuleId::Compose(rule) = violation.rule else {
                panic!("regla inesperada {:?}", violation.rule);
            };
            (rule, violation.line, violation.subject)
        })
        .collect()
}

#[test]
fn every_complete_service_needs_a_memory_limit() {
    let source = "x-limits: &limits
  deploy:
    resources:
      limits:
        memory: 256M
services:
  api:
    image: api:1
  worker:
    <<: *limits
    build: .
  cache:
    image: redis:7
    mem_limit: 128m
  partial:
    environment:
      A: 1
  child:
    extends:
      service: api
";
    assert_eq!(
        found(source),
        vec![(ComposeRule::MemoryLimit, 7, "api".to_owned())]
    );
}

#[test]
fn healthchecks_need_a_start_period_and_reasonable_intervals() {
    let source = "services:
  db:
    image: postgres:16
    mem_limit: 1g
    healthcheck:
      test: [\"CMD\", \"pg_isready\"]
      interval: 1s
  web:
    image: web:1
    mem_limit: 1g
    healthcheck:
      test: curl -f http://localhost
      interval: 10s
      timeout: 10s
      start_period: 20s
  ok:
    image: ok:1
    mem_limit: 1g
    healthcheck:
      test: [\"CMD\", \"true\"]
      interval: 30s
      timeout: 5s
      start_period: 1m
  off:
    image: off:1
    mem_limit: 1g
    healthcheck:
      disable: true
  none:
    image: none:1
    mem_limit: 1g
    healthcheck:
      test: [\"NONE\"]
";
    assert_eq!(
        found(source),
        vec![
            (ComposeRule::HealthcheckStartPeriod, 5, "db".to_owned()),
            (ComposeRule::HealthcheckInterval, 7, "db".to_owned()),
            (ComposeRule::HealthcheckInterval, 13, "web".to_owned()),
        ]
    );
}

#[test]
fn reads_flow_mappings_and_ignores_files_without_services() {
    assert_eq!(
        found("services: {api: {image: \"api:1\", deploy: {resources: {limits: {memory: 1g}}}}}"),
        Vec::new()
    );
    assert_eq!(found("version: '3'\n"), Vec::new());
    assert_eq!(found(""), Vec::new());
}
