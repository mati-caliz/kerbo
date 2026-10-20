use std::path::Path;

use super::{Dependency, Format, first_line_containing, line_of};
use crate::osv::Ecosystem;
use crate::test_support::TempTree;

fn parse(file_name: &str, text: &str) -> Vec<(String, String, usize)> {
    let format = Format::of(file_name).expect("el nombre es de un lockfile");
    format
        .parse(Path::new(file_name), text)
        .into_iter()
        .map(
            |Dependency {
                 name, version, line, ..
             }| (name, version, line),
        )
        .collect()
}

fn nothing() -> Vec<(String, String, usize)> {
    Vec::new()
}

fn found(name: &str, version: &str, line: usize) -> (String, String, usize) {
    (name.to_owned(), version.to_owned(), line)
}

#[test]
fn recognizes_every_lockfile_by_name() {
    for name in [
        "Cargo.lock",
        "package-lock.json",
        "npm-shrinkwrap.json",
        "pnpm-lock.yaml",
        "yarn.lock",
        "go.mod",
        "go.sum",
        "poetry.lock",
        "uv.lock",
        "Pipfile.lock",
        "requirements.txt",
        "requirements-dev.txt",
        "pom.xml",
        "gradle.lockfile",
    ] {
        assert!(Format::of(name).is_some(), "{name}");
    }
    assert_eq!(Format::of("package.json"), None);
    assert_eq!(Format::of("requirements.in"), None);
}

#[test]
fn reads_toml_lockfiles_of_cargo_poetry_and_uv() {
    let cargo = "version = 3\n\n[[package]]\nname = \"regex\"\nversion = \"1.5.4\"\nsource = \"registry+https://github.com/rust-lang/crates.io-index\"\ndependencies = [\n \"memchr\",\n]\n\n[[package]]\nname = \"app\"\nversion = \"0.1.0\"\n";
    assert_eq!(
        parse("Cargo.lock", cargo),
        [found("regex", "1.5.4", 4), found("app", "0.1.0", 12)]
    );
    let uv = "[[package]]\nname = \"pydantic\"\nversion = \"2.10.0\"\ndependencies = [\n    { name = \"annotated-types\" },\n]\n\n[package.optional-dependencies]\nemail = [{ name = \"email-validator\" }]\n";
    assert_eq!(parse("uv.lock", uv), [found("pydantic", "2.10.0", 2)]);
}

#[test]
fn reads_npm_lockfiles_in_every_version() {
    let modern = r#"{
  "lockfileVersion": 3,
  "packages": {
    "": { "name": "app", "version": "1.0.0" },
    "node_modules/lodash": {
      "version": "4.17.20"
    },
    "node_modules/a/node_modules/@scope/b": {
      "version": "2.0.0"
    },
    "node_modules/bundled": {
      "version": "1.0.0",
      "inBundle": true
    },
    "node_modules/linked": {
      "resolved": "packages/linked",
      "link": true
    },
    "node_modules/git": {
      "version": "git+https://example.com/repo.git"
    }
  }
}"#;
    assert_eq!(
        parse("package-lock.json", modern),
        [found("@scope/b", "2.0.0", 8), found("lodash", "4.17.20", 5)]
    );
    let legacy = r#"{
  "lockfileVersion": 1,
  "dependencies": {
    "express": {
      "version": "4.16.0",
      "dependencies": {
        "debug": {
          "version": "2.6.9"
        }
      }
    }
  }
}"#;
    assert_eq!(
        parse("package-lock.json", legacy),
        [found("express", "4.16.0", 4), found("debug", "2.6.9", 7)]
    );
    assert_eq!(parse("package-lock.json", "no es json"), nothing());
}

#[test]
fn reads_pnpm_lockfiles_old_and_new() {
    let legacy = "lockfileVersion: 5.4\n\npackages:\n\n  /lodash/4.17.20:\n    resolution: {integrity: sha512-x}\n\n  /@babel/core/7.0.0_react@17.0.0:\n    dev: true\n";
    assert_eq!(
        parse("pnpm-lock.yaml", legacy),
        [found("lodash", "4.17.20", 5), found("@babel/core", "7.0.0", 8)]
    );
    let modern = "lockfileVersion: '9.0'\n\nimporters:\n\n  .:\n    dependencies:\n      lodash:\n        specifier: ^4.17.20\n\npackages:\n\n  lodash@4.17.20:\n    resolution: {integrity: sha512-x}\n\n  '@vue/shared@3.0.0(vue@3.0.0)':\n    resolution: {integrity: sha512-y}\n\nsnapshots:\n\n  lodash@4.17.21: {}\n";
    assert_eq!(
        parse("pnpm-lock.yaml", modern),
        [found("lodash", "4.17.20", 12), found("@vue/shared", "3.0.0", 15)]
    );
}

#[test]
fn reads_yarn_lockfiles_classic_and_berry() {
    let classic = "# yarn lockfile v1\n\n\"@babel/code-frame@^7.0.0\", \"@babel/code-frame@^7.8.3\":\n  version \"7.8.3\"\n  resolved \"https://registry\"\n  dependencies:\n    \"@babel/highlight\" \"^7.8.3\"\n\nabbrev@1:\n  version \"1.1.1\"\n";
    assert_eq!(
        parse("yarn.lock", classic),
        [
            found("@babel/code-frame", "7.8.3", 3),
            found("abbrev", "1.1.1", 9)
        ]
    );
    let berry = "__metadata:\n  version: 6\n\n\"app@workspace:.\":\n  version: 0.0.0-use.local\n\n\"lodash@npm:^4.17.20\":\n  version: 4.17.20\n  resolution: \"lodash@npm:4.17.20\"\n";
    assert_eq!(parse("yarn.lock", berry), [found("lodash", "4.17.20", 7)]);
}

#[test]
fn reads_go_modules_with_replacements_and_checksums() {
    let module = "module example.com/app\n\ngo 1.21\n\nrequire github.com/a/b v1.2.3\n\nrequire (\n\tgolang.org/x/crypto v0.1.0 // indirect\n\tgithub.com/old/thing v1.0.0\n\tgithub.com/local/thing v1.0.0\n)\n\nreplace github.com/old/thing => github.com/new/thing v1.1.0\n\nreplace (\n\tgithub.com/local/thing => ./local\n)\n";
    assert_eq!(
        parse("go.mod", module),
        [
            found("github.com/a/b", "1.2.3", 5),
            found("golang.org/x/crypto", "0.1.0", 8),
            found("github.com/new/thing", "1.1.0", 9),
        ]
    );
    let sums = "github.com/a/b v1.2.3 h1:abc=\ngithub.com/a/b v1.2.3/go.mod h1:def=\ngithub.com/a/b v1.2.3 h1:abc=\n";
    assert_eq!(parse("go.sum", sums), [found("github.com/a/b", "1.2.3", 1)]);
}

#[test]
fn reads_only_pinned_python_requirements() {
    let requirements = "# dev\n-r base.txt\nDjango==4.2.1 ; python_version >= \"3.8\"\nrequests[socks] == 2.31.0 \\\n    --hash=sha256:abc\nflask>=2.0\nnumpy==1.*\ncelery==5.0,!=5.0.1\npkg @ https://example.com/pkg.whl\nPyYAML===6.0\n";
    assert_eq!(
        parse("requirements.txt", requirements),
        [
            found("django", "4.2.1", 3),
            found("requests", "2.31.0", 4),
            found("pyyaml", "6.0", 10),
        ]
    );
    let pipfile = "{\n  \"default\": {\n    \"Django\": {\n      \"version\": \"==4.2.1\"\n    }\n  },\n  \"develop\": {\n    \"pytest\": {\n      \"version\": \"==7.0.0\"\n    }\n  }\n}\n";
    assert_eq!(
        parse("Pipfile.lock", pipfile),
        [found("django", "4.2.1", 3), found("pytest", "7.0.0", 8)]
    );
    assert_eq!(parse("Pipfile.lock", "["), nothing());
}

#[test]
fn reads_gradle_lockfiles() {
    let lockfile = "# This is a Gradle generated file\ncom.google.guava:guava:18.0=compileClasspath,runtimeClasspath\nempty=annotationProcessor\n";
    assert_eq!(
        parse("gradle.lockfile", lockfile),
        [found("com.google.guava:guava", "18.0", 2)]
    );
}

#[test]
fn reads_poms_with_properties_and_local_parents() {
    let tree = TempTree::new("osv-pom");
    tree.write(
        "pom.xml",
        "<project>\n  <groupId>org.example</groupId>\n  <artifactId>parent</artifactId>\n  <version>1.0</version>\n  <properties>\n    <guava.version>18.0</guava.version>\n  </properties>\n  <dependencyManagement>\n    <dependencies>\n      <dependency>\n        <groupId>junit</groupId>\n        <artifactId>junit</artifactId>\n        <version>4.12</version>\n      </dependency>\n    </dependencies>\n  </dependencyManagement>\n  <dependencies>\n    <dependency>\n      <groupId>commons-io</groupId>\n      <artifactId>commons-io</artifactId>\n      <version>2.4</version>\n    </dependency>\n  </dependencies>\n</project>\n",
    );
    let child = "<?xml version=\"1.0\"?>\n<project>\n  <!-- <dependency><groupId>x</groupId></dependency> -->\n  <artifactId>child</artifactId>\n  <parent>\n    <groupId>org.example</groupId>\n    <artifactId>parent</artifactId>\n    <version>1.0</version>\n  </parent>\n  <dependencies>\n    <dependency>\n      <groupId>com.google.guava</groupId>\n      <artifactId>guava</artifactId>\n      <version>${guava.version}</version>\n    </dependency>\n    <dependency>\n      <groupId>junit</groupId>\n      <artifactId>junit</artifactId>\n    </dependency>\n    <dependency>\n      <groupId>${project.groupId}</groupId>\n      <artifactId>sibling</artifactId>\n      <version>${project.version}</version>\n    </dependency>\n    <dependency>\n      <groupId>unknown</groupId>\n      <artifactId>unresolved</artifactId>\n      <version>${missing.version}</version>\n    </dependency>\n  </dependencies>\n  <build><plugins><plugin><dependencies><dependency><groupId>p</groupId><artifactId>p</artifactId><version>1</version></dependency></dependencies></plugin></plugins></build>\n</project>\n";
    tree.write("child/pom.xml", child);
    let format = Format::of("pom.xml").expect("pom.xml es un formato");
    let dependencies: Vec<(String, String, usize)> = format
        .parse(&tree.path().join("child/pom.xml"), child)
        .into_iter()
        .map(|dependency| (dependency.name, dependency.version, dependency.line))
        .collect();
    assert_eq!(
        dependencies,
        [
            found("com.google.guava:guava", "18.0", 11),
            found("junit:junit", "4.12", 16),
            found("org.example:sibling", "1.0", 20),
            found("commons-io:commons-io", "2.4", 5),
        ]
    );
    let orphan = child.replace(
        "<version>1.0</version>\n  </parent>",
        "<version>9.9</version>\n  </parent>",
    );
    let alone = format.parse(&tree.path().join("child/pom.xml"), &orphan);
    assert!(
        alone
            .iter()
            .all(|dependency| dependency.ecosystem == Ecosystem::Maven)
    );
    assert_eq!(alone.len(), 1);
}

#[test]
fn counts_lines_from_one() {
    assert_eq!(line_of("a\nb\nc", 4), 3);
    assert_eq!(first_line_containing("a\nb\nc", "b"), 2);
    assert_eq!(first_line_containing("a", "z"), 1);
}
