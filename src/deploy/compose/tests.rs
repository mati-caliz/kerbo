use tree_sitter::Parser;

use super::violations;
use crate::language::Language;
use crate::rules::{ComposeRule, RuleId};

const RESOURCE_AND_HEALTH_RULES: [ComposeRule; 3] = [
    ComposeRule::MemoryLimit,
    ComposeRule::HealthcheckStartPeriod,
    ComposeRule::HealthcheckInterval,
];
const SERVICE_RULES: [ComposeRule; 4] = [
    ComposeRule::CpuLimit,
    ComposeRule::Restart,
    ComposeRule::ImageTag,
    ComposeRule::DependsOnHealthy,
];

fn found(source: &str) -> Vec<(ComposeRule, usize, String)> {
    found_of(source, &RESOURCE_AND_HEALTH_RULES)
}

fn found_of(source: &str, rules: &[ComposeRule]) -> Vec<(ComposeRule, usize, String)> {
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
        .filter(|(rule, _, _)| rules.contains(rule))
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

#[test]
fn services_need_cpu_limits_a_restart_policy_pinned_images_and_healthy_dependencies() {
    let source = "services:
  db:
    image: postgres
    cpus: 1
    restart: always
    healthcheck:
      test: [\"CMD\", \"pg_isready\"]
  cache:
    image: redis:latest
    deploy:
      resources:
        limits:
          cpus: \"0.5\"
      restart_policy:
        condition: on-failure
  api:
    build: .
    image: registry:5000/team/api@sha256:abc
    depends_on:
      db:
        condition: service_started
      cache:
        condition: service_started
  worker:
    image: ${WORKER_IMAGE}
    cpus: 1
    restart: unless-stopped
    depends_on: [db, cache]
  ready:
    image: localhost:5000/ready:1.2
    cpus: 1
    restart: \"no\"
    depends_on:
      db:
        condition: service_healthy
  base:
    extends:
      service: api
";
    assert_eq!(
        found_of(source, &SERVICE_RULES),
        vec![
            (ComposeRule::ImageTag, 3, "postgres".to_owned()),
            (ComposeRule::ImageTag, 9, "redis:latest".to_owned()),
            (ComposeRule::CpuLimit, 16, "api".to_owned()),
            (ComposeRule::Restart, 16, "api".to_owned()),
            (ComposeRule::DependsOnHealthy, 20, "db".to_owned()),
            (ComposeRule::DependsOnHealthy, 28, "db".to_owned()),
        ]
    );
}
