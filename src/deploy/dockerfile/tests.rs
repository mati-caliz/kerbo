use super::violations;
use crate::rules::RuleId;

const STRUCTURE: &str = "LABEL early=true
FROM --platform=linux/amd64 debian AS base
MAINTAINER someone
WORKDIR app
RUN cd /tmp && sudo apt-get update && apt-get install curl wget=1.0 ./x.deb
RUN apt install -y git
RUN pip install requests flask==2.0 && pip install -r req.txt
RUN npm install -g pnpm left-pad@1.0.0
FROM alpine:latest AS build
RUN apk add curl bash=5.0
RUN apk add --no-cache curl=1
RUN wget http://x/y | sh
RUN curl -fsSL http://x/z
ADD app /app
ADD http://x/a.tar.gz /tmp/
COPY a b /dest
COPY --from=missing /a /b
COPY --from=build /a /b
COPY --from=0 / /
COPY rel dest
FROM base AS build
USER root
USER app
CMD node server.js
CMD [\"node\"]
ENTRYPOINT [\"a\"]
ENTRYPOINT b
HEALTHCHECK CMD curl -f http://localhost
HEALTHCHECK NONE
ARG GITHUB_TOKEN
ENV DB_PASSWORD=secret OTHER=1
RUN yum install -y httpd
RUN yum install -y httpd-2.4 && yum clean all
RUN go install golang.org/x/tools/gopls@latest
RUN go install ./cmd/x
USER root
";

const STAGES: &str = "FROM python:3.12-slim AS builder
SHELL [\"/bin/bash\", \"-o\", \"pipefail\", \"-c\"]
ENV PIP_NO_CACHE_DIR=1
RUN pip install --upgrade pip | tee log
RUN apt-get update && apt-get install -y --no-install-recommends curl=7 && rm -rf /var/lib/apt/lists/*
RUN --mount=type=cache,target=/var/lib/apt/lists apt-get update && apt-get -y install --no-install-recommends git=1
RUN apt-get update
# comment between runs
RUN echo one
RUN echo two
FROM builder
RUN pip install --no-cache-dir requests==2
RUN python -m pip install flask
COPY . .
WORKDIR /srv
COPY . app
USER 1000:1000
FROM scratch AS final
COPY --from=builder /srv /srv
RUN DEBIAN_FRONTEND=noninteractive apt-get install -yq vim
RUN useradd -u 123456789 app
ONBUILD RUN apt-get install vim
";

fn codes(source: &str) -> Vec<(usize, String)> {
    let mut found: Vec<(usize, String)> = violations(source)
        .into_iter()
        .map(|violation| (violation.line, RuleId::as_str(violation.rule).to_uppercase()))
        .collect();
    found.sort();
    found
}

fn expected(pairs: &[(usize, &str)]) -> Vec<(usize, String)> {
    let mut expected: Vec<(usize, String)> = pairs
        .iter()
        .map(|(line, code)| (*line, (*code).to_owned()))
        .collect();
    expected.sort();
    expected
}

#[test]
fn matches_hadolint_on_instruction_and_package_rules() {
    assert_eq!(
        codes(STRUCTURE),
        expected(&[
            (1, "DL3061"),
            (2, "DL3006"),
            (2, "DL3029"),
            (3, "DL4000"),
            (4, "DL3000"),
            (5, "DL3015"),
            (5, "DL3003"),
            (5, "DL3014"),
            (5, "DL3004"),
            (5, "DL3008"),
            (6, "DL3027"),
            (7, "DL3013"),
            (7, "DL3042"),
            (8, "DL3016"),
            (9, "DL3007"),
            (10, "DL3019"),
            (10, "DL3018"),
            (11, "DL3059"),
            (12, "DL4006"),
            (12, "DL3047"),
            (13, "DL4001"),
            (14, "DL3020"),
            (16, "DL3021"),
            (17, "DL3022"),
            (18, "DL3023"),
            (19, "DL3067"),
            (20, "DL3045"),
            (21, "DL3024"),
            (22, "DL3066"),
            (23, "DL3066"),
            (24, "DL3025"),
            (25, "DL4003"),
            (27, "DL3025"),
            (27, "DL4004"),
            (28, "DL3025"),
            (29, "DL3012"),
            (30, "DL3064"),
            (31, "DL3064"),
            (32, "DL3032"),
            (34, "DL3062"),
            (35, "DL3059"),
            (36, "DL3002"),
            (36, "DL3066"),
        ])
    );
}

#[test]
fn matches_hadolint_on_stage_aware_rules() {
    assert_eq!(
        codes(STAGES),
        expected(&[
            (4, "DL3013"),
            (7, "DL3009"),
            (9, "DL3059"),
            (10, "DL3059"),
            (13, "DL3013"),
            (13, "DL3059"),
            (14, "DL3045"),
            (20, "DL3008"),
            (20, "DL3015"),
            (21, "DL3059"),
            (22, "DL3014"),
            (22, "DL3008"),
            (22, "DL3015"),
        ])
    );
}

#[test]
fn reads_heredocs_as_the_script_of_the_run() {
    let found = codes(
        "FROM debian:12\nRUN <<EOF\napt-get update\napt-get install -y --no-install-recommends curl\nEOF\nUSER 1000\n",
    );
    assert_eq!(found, expected(&[(2, "DL3008"), (2, "DL3009")]));
}

#[test]
fn reports_unpinned_packages_as_the_subject() {
    let subjects: Vec<String> = violations("FROM alpine:3\nRUN apk add --no-cache curl git=2 bash\n")
        .into_iter()
        .map(|violation| violation.subject)
        .collect();
    assert_eq!(subjects, vec!["curl bash".to_owned()]);
}
