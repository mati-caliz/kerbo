use crate::i18n::catalog::RuleText;
use crate::rules::DockerfileRule;

const UNPINNED_WHY: &str = "Without a version, every build installs whatever was published last: two builds of the same commit do not give the same image.";
const LATEST_WHY: &str =
    "Without a fixed version, the same Dockerfile builds a different image the day a new base is published.";

pub const fn dockerfile(rule: DockerfileRule) -> RuleText {
    match rule {
        DockerfileRule::Dl3000 => RuleText::new(
            "WORKDIR with a relative path",
            "use an absolute path in WORKDIR",
            "A relative path depends on the previous WORKDIR and on the base image: it changes without anyone touching this Dockerfile.",
            "WORKDIR app becomes WORKDIR /app.",
        ),
        DockerfileRule::Dl3002 => RuleText::new(
            "The stage ends up running as root",
            "end the stage with an unprivileged USER",
            "If the process runs as root, any vulnerability in it is root in the container and one step closer to the host.",
            "After installing, RUN useradd -r app and USER app.",
        ),
        DockerfileRule::Dl3003 => RuleText::new(
            "cd inside a RUN",
            "use WORKDIR to change directory",
            "A cd only applies to that RUN: the next one starts somewhere else and it is easy to forget.",
            "RUN cd /app && make becomes WORKDIR /app and RUN make.",
        ),
        DockerfileRule::Dl3004 => RuleText::new(
            "sudo inside a RUN",
            "run the step as root with USER, or use gosu in the entrypoint",
            "Every RUN already runs as the current USER; sudo adds a setuid binary and behavior that depends on its configuration.",
            "RUN sudo apt-get install ... becomes USER root, the RUN, and USER app again.",
        ),
        DockerfileRule::Dl3006 => RuleText::new(
            "Base image without a tag",
            "pin the version of the base image",
            LATEST_WHY,
            "FROM python becomes FROM python:3.12-slim.",
        ),
        DockerfileRule::Dl3007 => RuleText::new(
            "Base image with the latest tag",
            "pin a concrete version or a digest",
            LATEST_WHY,
            "FROM node:latest becomes FROM node:22.11-alpine.",
        ),
        DockerfileRule::Dl3008 => RuleText::new(
            "apt-get install without pinned versions",
            "pin every package with package=version",
            UNPINNED_WHY,
            "apt-get install -y curl becomes apt-get install -y curl=8.5.0-2.",
        ),
        DockerfileRule::Dl3009 => RuleText::new(
            "The apt lists stay in the image",
            "delete /var/lib/apt/lists in the same RUN that runs apt-get update",
            "The lists weigh tens of MB and, once they are in a layer, no later RUN removes them from the image.",
            "RUN apt-get update && apt-get install -y curl && rm -rf /var/lib/apt/lists/*",
        ),
        DockerfileRule::Dl3012 => RuleText::new(
            "More than one HEALTHCHECK in the same stage",
            "keep a single HEALTHCHECK",
            "Only the last one applies: the others confuse the reader and check nothing.",
            "Delete the extra HEALTHCHECK instructions and keep the one that reflects the service.",
        ),
        DockerfileRule::Dl3013 => RuleText::new(
            "pip install without pinned versions",
            "pin versions with package==version or install from a requirements file with versions",
            UNPINNED_WHY,
            "pip install requests becomes pip install requests==2.32.3.",
        ),
        DockerfileRule::Dl3014 => RuleText::new(
            "apt-get install without -y",
            "add -y so it does not ask for confirmation",
            "Without -y the build waits for an answer that never comes and fails.",
            "apt-get install curl becomes apt-get install -y curl.",
        ),
        DockerfileRule::Dl3015 => RuleText::new(
            "apt-get install brings recommended packages",
            "add --no-install-recommends",
            "Recommended packages add things nobody asked for: more weight, more attack surface and longer builds.",
            "apt-get install -y curl becomes apt-get install -y --no-install-recommends curl.",
        ),
        DockerfileRule::Dl3016 => RuleText::new(
            "npm install without pinned versions",
            "pin the version with package@version",
            UNPINNED_WHY,
            "npm install -g pnpm becomes npm install -g pnpm@9.12.0.",
        ),
        DockerfileRule::Dl3018 => RuleText::new(
            "apk add without pinned versions",
            "pin every package with package=version",
            UNPINNED_WHY,
            "apk add curl becomes apk add curl=8.11.0-r2.",
        ),
        DockerfileRule::Dl3019 => RuleText::new(
            "apk add without --no-cache",
            "use apk add --no-cache",
            "Without --no-cache the package index stays in the image, or you need an update and a manual rm.",
            "apk update && apk add curl becomes apk add --no-cache curl.",
        ),
        DockerfileRule::Dl3020 => RuleText::new(
            "ADD to copy files or folders",
            "use COPY; ADD only to extract a tar or download a URL",
            "ADD extracts archives and downloads URLs on its own: with plain files it does what COPY does, with surprises.",
            "ADD ./app /app becomes COPY ./app /app.",
        ),
        DockerfileRule::Dl3021 => RuleText::new(
            "COPY of several files to a destination that does not end in /",
            "end the destination with /",
            "With several sources the destination must be a folder; without the slash the build fails.",
            "COPY a.txt b.txt /dest becomes COPY a.txt b.txt /dest/.",
        ),
        DockerfileRule::Dl3022 => RuleText::new(
            "COPY --from a stage that is not defined",
            "point to an alias defined earlier with FROM ... AS",
            "If the name is not an earlier stage, Docker looks it up as an image and copies from something nobody defined.",
            "FROM golang:1.23 AS build ... COPY --from=build /out/app /app.",
        ),
        DockerfileRule::Dl3023 => RuleText::new(
            "COPY --from the same stage",
            "copy from another stage",
            "A stage cannot copy from itself: the build fails.",
            "COPY --from=build goes in the final stage, not inside build.",
        ),
        DockerfileRule::Dl3024 => RuleText::new(
            "Two stages with the same alias",
            "give every stage a unique name",
            "With repeated names, COPY --from points to a different stage than you think.",
            "FROM node:22 AS build and FROM golang:1.23 AS build become build-web and build-api.",
        ),
        DockerfileRule::Dl3025 => RuleText::new(
            "CMD or ENTRYPOINT in shell form",
            "use the JSON form: CMD [\"program\", \"argument\"]",
            "In shell form the process runs under /bin/sh -c: it does not get SIGTERM and the container does not shut down cleanly.",
            "CMD node server.js becomes CMD [\"node\", \"server.js\"].",
        ),
        DockerfileRule::Dl3027 => RuleText::new(
            "apt instead of apt-get",
            "use apt-get or apt-cache",
            "apt is meant for people: its output and interface change between versions and are not stable for a script.",
            "apt install -y curl becomes apt-get install -y curl.",
        ),
        DockerfileRule::Dl3029 => RuleText::new(
            "FROM with a fixed --platform",
            "remove --platform or use $BUILDPLATFORM or $TARGETPLATFORM",
            "A fixed --platform prevents building the image for other architectures and emulates needlessly on every build.",
            "FROM --platform=linux/amd64 node:22 becomes FROM --platform=$BUILDPLATFORM node:22.",
        ),
        DockerfileRule::Dl3032 => RuleText::new(
            "yum install without yum clean all afterwards",
            "end the RUN with yum clean all",
            "The yum cache stays in the layer and adds weight that no later RUN removes.",
            "RUN yum install -y curl && yum clean all",
        ),
        DockerfileRule::Dl3042 => RuleText::new(
            "pip install keeps its cache in the image",
            "add --no-cache-dir or ENV PIP_NO_CACHE_DIR=1",
            "The pip cache only helps the next install on the same machine: in the image it is dead weight.",
            "pip install -r requirements.txt becomes pip install --no-cache-dir -r requirements.txt.",
        ),
        DockerfileRule::Dl3045 => RuleText::new(
            "COPY to a relative destination without WORKDIR",
            "set WORKDIR first or use an absolute destination",
            "Without WORKDIR, the relative path depends on the base image and may end up in /.",
            "COPY . app becomes WORKDIR /app and COPY . .",
        ),
        DockerfileRule::Dl3047 => RuleText::new(
            "wget without controlling the progress bar",
            "use wget --progress=dot:giga, -q or -nv",
            "The wget progress bar floods the build log with thousands of lines.",
            "wget https://x/file.tar.gz becomes wget -q https://x/file.tar.gz.",
        ),
        DockerfileRule::Dl3059 => RuleText::new(
            "Several consecutive RUN instructions that can be merged",
            "join consecutive RUN instructions with &&",
            "Every RUN is one more layer: more weight, more time and a cache that is invalidated piece by piece.",
            "RUN apt-get update and RUN apt-get install -y curl become RUN apt-get update && apt-get install -y curl.",
        ),
        DockerfileRule::Dl3061 => RuleText::new(
            "Instruction before the first FROM",
            "start the Dockerfile with FROM or ARG",
            "Before the first FROM only ARG and comments are valid: anything else belongs to no stage and the build fails.",
            "Move that LABEL or ENV below the FROM.",
        ),
        DockerfileRule::Dl3062 => RuleText::new(
            "go install or go run without a pinned version",
            "pin the version with package@vX.Y.Z",
            UNPINNED_WHY,
            "go install golang.org/x/tools/gopls@latest becomes go install golang.org/x/tools/gopls@v0.16.2.",
        ),
        DockerfileRule::Dl3064 => RuleText::new(
            "ARG or ENV named like sensitive data",
            "pass secrets with RUN --mount=type=secret, not with ARG or ENV",
            "ARG and ENV stay in the image history: anyone who pulls it reads the value with docker history.",
            "ARG NPM_TOKEN becomes RUN --mount=type=secret,id=npm_token ...",
        ),
        DockerfileRule::Dl3066 => RuleText::new(
            "USER with a name instead of a numeric UID",
            "use the numeric UID, and the GID if needed, in USER",
            "Kubernetes and other orchestrators check runAsNonRoot with the number: with a name they cannot tell whether it is root.",
            "USER app becomes USER 10001:10001.",
        ),
        DockerfileRule::Dl3067 => RuleText::new(
            "COPY --from copies the entire filesystem",
            "copy only the folders or files you need",
            "Copying / from another stage brings binaries, caches and system files that overwrite those of the current stage.",
            "COPY --from=build / / becomes COPY --from=build /out/app /usr/local/bin/app.",
        ),
        DockerfileRule::Dl4000 => RuleText::new(
            "MAINTAINER is deprecated",
            "use LABEL maintainer=...",
            "MAINTAINER is no longer part of the specification: a label that tools read replaces it.",
            "MAINTAINER Ana <ana@example.com> becomes LABEL maintainer=\"ana@example.com\".",
        ),
        DockerfileRule::Dl4001 => RuleText::new(
            "wget and curl in the same stage",
            "use only one of the two tools",
            "Installing two tools for the same job adds weight and two different behaviors on errors.",
            "If you already use curl, wget -O x URL becomes curl -fsSLo x URL.",
        ),
        DockerfileRule::Dl4003 => RuleText::new(
            "More than one CMD in the same stage",
            "keep a single CMD",
            "Only the last CMD applies: the earlier ones never run and confuse the reader.",
            "Delete the extra CMD instructions.",
        ),
        DockerfileRule::Dl4004 => RuleText::new(
            "More than one ENTRYPOINT in the same stage",
            "keep a single ENTRYPOINT",
            "Only the last ENTRYPOINT applies: the earlier ones never run and confuse the reader.",
            "Delete the extra ENTRYPOINT instructions.",
        ),
        DockerfileRule::Dl4006 => RuleText::new(
            "RUN with a pipe and no pipefail",
            "put SHELL [\"/bin/bash\", \"-o\", \"pipefail\", \"-c\"] before the RUN",
            "Without pipefail, in curl ... | sh only the last command counts: if the download fails, the build goes on as if nothing happened.",
            "SHELL [\"/bin/bash\", \"-o\", \"pipefail\", \"-c\"] and then RUN curl -fsSL URL | sh.",
        ),
    }
}
