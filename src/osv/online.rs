use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use super::lockfiles::Dependency;
use super::record::Record;
use super::responses::{Listed, Responses};
use super::transport::Transport;
use crate::i18n::OsvProblem;

const QUERY_URL: &str = "https://api.osv.dev/v1/querybatch";
const RECORD_URL: &str = "https://api.osv.dev/v1/vulns/";
const BATCH_SIZE: usize = 1000;
const KEY_SEPARATOR: char = '\t';

#[derive(Debug, Serialize)]
struct PackageQuery<'query> {
    package: PackageName<'query>,
    version: &'query str,
    #[serde(skip_serializing_if = "Option::is_none")]
    page_token: Option<String>,
}

#[derive(Debug, Serialize)]
struct PackageName<'query> {
    name: &'query str,
    ecosystem: &'static str,
}

#[derive(Debug, Serialize)]
struct Batch<'query> {
    queries: Vec<PackageQuery<'query>>,
}

#[derive(Debug, Deserialize)]
struct BatchResponse {
    #[serde(default)]
    results: Vec<BatchResult>,
}

#[derive(Debug, Default, Deserialize)]
struct BatchResult {
    #[serde(default)]
    vulns: Vec<Listed>,
    next_page_token: Option<String>,
}

pub fn query_key(dependency: &Dependency) -> String {
    format!(
        "{}{KEY_SEPARATOR}{}{KEY_SEPARATOR}{}",
        dependency.ecosystem.as_str(),
        dependency.name,
        dependency.version
    )
}

pub fn lookup(
    dependencies: &[&Dependency],
    transport: &dyn Transport,
    responses: &mut Responses,
) -> Result<BTreeMap<String, Vec<Record>>, OsvProblem> {
    let mut unique: BTreeMap<String, &Dependency> = BTreeMap::new();
    for dependency in dependencies {
        unique.entry(query_key(dependency)).or_insert(dependency);
    }
    let mut listed: BTreeMap<String, Vec<Listed>> = BTreeMap::new();
    let mut missing = Vec::new();
    for (key, dependency) in &unique {
        match responses.listed(key) {
            Some(found) => {
                listed.insert(key.clone(), found);
            }
            None => missing.push((key.clone(), *dependency)),
        }
    }
    for chunk in missing.chunks(BATCH_SIZE) {
        for ((key, _), vulns) in chunk.iter().zip(query_batch(chunk, transport)?) {
            responses.remember_listed(key.clone(), vulns.clone());
            listed.insert(key.clone(), vulns);
        }
    }
    let records = fetch_records(listed.values().flatten(), transport, responses)?;
    Ok(listed
        .into_iter()
        .map(|(key, vulns)| {
            let found = vulns
                .iter()
                .filter_map(|vuln| records.get(&vuln.id).cloned())
                .collect();
            (key, found)
        })
        .collect())
}

fn query_batch(
    chunk: &[(String, &Dependency)],
    transport: &dyn Transport,
) -> Result<Vec<Vec<Listed>>, OsvProblem> {
    let queries = chunk
        .iter()
        .map(|(_, dependency)| package_query(dependency, None))
        .collect();
    let mut results = post(transport, &Batch { queries })?;
    results.resize_with(chunk.len(), BatchResult::default);
    results
        .into_iter()
        .zip(chunk)
        .map(|(result, (_, dependency))| remaining_pages(result, dependency, transport))
        .collect()
}

fn remaining_pages(
    first: BatchResult,
    dependency: &Dependency,
    transport: &dyn Transport,
) -> Result<Vec<Listed>, OsvProblem> {
    let mut vulns = first.vulns;
    let mut token = first.next_page_token;
    while let Some(page_token) = token.take().filter(|token| !token.is_empty()) {
        let page = post(
            transport,
            &Batch {
                queries: vec![package_query(dependency, Some(page_token))],
            },
        )?;
        let next = page.into_iter().next().unwrap_or_default();
        vulns.extend(next.vulns);
        token = next.next_page_token;
    }
    Ok(vulns)
}

fn package_query(dependency: &Dependency, page_token: Option<String>) -> PackageQuery<'_> {
    PackageQuery {
        package: PackageName {
            name: &dependency.name,
            ecosystem: dependency.ecosystem.as_str(),
        },
        version: &dependency.version,
        page_token,
    }
}

fn post(transport: &dyn Transport, batch: &Batch<'_>) -> Result<Vec<BatchResult>, OsvProblem> {
    let body = serde_json::to_vec(batch).map_err(|error| OsvProblem::InvalidResponse(error.to_string()))?;
    let bytes = transport
        .post_json(QUERY_URL, &body)
        .map_err(OsvProblem::Unavailable)?;
    serde_json::from_slice::<BatchResponse>(&bytes)
        .map(|response| response.results)
        .map_err(|error| OsvProblem::InvalidResponse(error.to_string()))
}

fn fetch_records<'listed>(
    listed: impl Iterator<Item = &'listed Listed>,
    transport: &dyn Transport,
    responses: &Responses,
) -> Result<BTreeMap<String, Record>, OsvProblem> {
    let mut records = BTreeMap::new();
    let mut wanted = BTreeSet::new();
    for vuln in listed {
        if records.contains_key(&vuln.id) {
            continue;
        }
        match responses.record(vuln) {
            Some(record) => {
                records.insert(vuln.id.clone(), record);
            }
            None => {
                wanted.insert(vuln.id.clone());
            }
        }
    }
    let wanted: Vec<String> = wanted
        .into_iter()
        .filter(|id| !records.contains_key(id))
        .collect();
    let urls: Vec<String> = wanted.iter().map(|id| format!("{RECORD_URL}{id}")).collect();
    let bodies = transport.get_all(&urls).map_err(OsvProblem::Unavailable)?;
    for (id, bytes) in wanted.iter().zip(bodies) {
        let record: Record =
            serde_json::from_slice(&bytes).map_err(|error| OsvProblem::InvalidResponse(error.to_string()))?;
        responses.remember_record(id, &bytes);
        records.insert(id.clone(), record);
    }
    Ok(records)
}
