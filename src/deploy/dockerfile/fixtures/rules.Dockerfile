FROM centos:7 AS rpm
ARG VERSION=2.4
ENV TOOL=4.1
RUN yum install httpd
RUN yum install -y httpd
RUN yum install -y httpd-2.4.57 nginx-${TOOL} curl-${MISSING} pkg-${VERSION} local.rpm
RUN yum module install -y nodejs:14 && yum module install -y ruby
RUN zypper install curl && zypper in -y vim=1.0 && zypper dup
RUN zypper install -y curl && zypper clean
RUN zypper clean && zypper install -y wget=1.2
RUN dnf install httpd && dnf clean all
RUN microdnf install -y nginx-1.20 && microdnf clean all
RUN dnf install -y nginx-1.20
RUN dnf group install -y "Development Tools" && dnf clean all
RUN rm -rf /var/cache/libdnf5* && dnf install -y zsh-5.9
RUN --mount=type=cache,target=/var/cache/libdnf5 dnf install -y fish-3.7

FROM node:20 AS scratch
RUN service nginx start && ps aux
RUN gem install bundler rails:7.1 -N --source http://x && gem install puma -v 6 && gem i rake
RUN useradd -u 1000000 big && useradd -l -u 1000000 ok && useradd -u 1000 small && useradd --uid 1000000 long
RUN ln -sf /bin/bash /bin/sh
COPY app.tar.gz /tmp/
COPY other.zip archive.tgz /opt/
RUN tar -xzf /tmp/app.tar.gz && unzip archive.tgz
RUN yarn install
EXPOSE 80 8080/tcp 70000 1000-80000 ${PORT}
ENV A=1 B=$A C=${A}x
ENV D=$A E=$Dx
ONBUILD ONBUILD RUN make
ONBUILD FROM base
ONBUILD MAINTAINER me

FROM --platform=$TARGETPLATFORM alpine:3.19 AS context
RUN yarn install && yarn cache clean
RUN yarn cache clean && yarn install

FROM --platform=${TARGETPLATFORM} scratch AS final
COPY final.tar /srv/
RUN tar xf /srv/final.tar
LABEL org.opencontainers.image.version=1 maintainer="me"
LABEL Version=1
LABEL com.docker.thing=x
LABEL a..b=1 c--d=2 ok/path_1=3
LABEL trailing-=1
LABEL "quoted.key"="v"
LABEL legacy value with spaces
