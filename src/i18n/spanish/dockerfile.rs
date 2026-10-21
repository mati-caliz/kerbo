use crate::i18n::catalog::RuleText;
use crate::rules::DockerfileRule;

const UNPINNED_WHY: &str = "Sin versión, cada build instala lo último que haya publicado: dos builds del mismo commit no dan la misma imagen.";
const LATEST_WHY: &str =
    "Sin una versión fija, el mismo Dockerfile arma otra imagen el día que publican una base nueva.";

const UNKNOWN_DOCKERFILE: RuleText = RuleText::new(
    "Regla de hadolint",
    "seguí la indicación de la regla",
    "hadolint la marca por defecto.",
    "Ver la wiki de hadolint.",
);
const DOCKERFILE_TEXTS: [RuleText; DockerfileRule::ALL.len()] = [
    RuleText::new(
        "WORKDIR con una ruta relativa",
        "usá una ruta absoluta en WORKDIR",
        "Una ruta relativa depende del WORKDIR anterior y de la imagen base: cambia sin que nadie toque este Dockerfile.",
        "WORKDIR app se vuelve WORKDIR /app.",
    ),
    RuleText::new(
        "La etapa termina corriendo como root",
        "terminá la etapa con un USER sin privilegios",
        "Si el proceso corre como root, cualquier vulnerabilidad en él es root en el contenedor y un paso más cerca del host.",
        "Después de instalar, RUN useradd -r app y USER app.",
    ),
    RuleText::new(
        "cd dentro de un RUN",
        "usá WORKDIR para cambiar de carpeta",
        "Un cd vale sólo para ese RUN: el siguiente arranca en otro lado y es fácil olvidarlo.",
        "RUN cd /app && make se vuelve WORKDIR /app y RUN make.",
    ),
    RuleText::new(
        "sudo dentro de un RUN",
        "corré el paso como root con USER, o usá gosu en el entrypoint",
        "Cada RUN ya corre como el USER actual; sudo suma un binario con setuid y un comportamiento que depende de su configuración.",
        "RUN sudo apt-get install ... se vuelve USER root, el RUN, y otra vez USER app.",
    ),
    RuleText::new(
        "Imagen base sin etiqueta",
        "fijá la versión de la imagen base",
        LATEST_WHY,
        "FROM python se vuelve FROM python:3.12-slim.",
    ),
    RuleText::new(
        "Imagen base con la etiqueta latest",
        "fijá una versión concreta o un digest",
        LATEST_WHY,
        "FROM node:latest se vuelve FROM node:22.11-alpine.",
    ),
    RuleText::new(
        "apt-get install sin versiones fijas",
        "fijá cada paquete con paquete=versión",
        UNPINNED_WHY,
        "apt-get install -y curl se vuelve apt-get install -y curl=8.5.0-2.",
    ),
    RuleText::new(
        "Quedan las listas de apt en la imagen",
        "borrá /var/lib/apt/lists en el mismo RUN que hace apt-get update",
        "Las listas pesan decenas de MB y, si quedan en una capa, ningún RUN posterior las saca de la imagen.",
        "RUN apt-get update && apt-get install -y curl && rm -rf /var/lib/apt/lists/*",
    ),
    RuleText::new(
        "Más de un HEALTHCHECK en la misma etapa",
        "dejá un solo HEALTHCHECK",
        "Sólo vale el último: los anteriores confunden a quien lee y no chequean nada.",
        "Borrá los HEALTHCHECK de más y quedate con el que refleja el servicio.",
    ),
    RuleText::new(
        "pip install sin versiones fijas",
        "fijá las versiones con paquete==versión o instalá desde un requirements con versiones",
        UNPINNED_WHY,
        "pip install requests se vuelve pip install requests==2.32.3.",
    ),
    RuleText::new(
        "apt-get install sin -y",
        "agregá -y para que no pida confirmación",
        "Sin -y el build queda esperando una respuesta que nunca llega y falla.",
        "apt-get install curl se vuelve apt-get install -y curl.",
    ),
    RuleText::new(
        "apt-get install trae los paquetes recomendados",
        "agregá --no-install-recommends",
        "Los recomendados suman paquetes que nadie pidió: más peso, más superficie de ataque y más tiempo de build.",
        "apt-get install -y curl se vuelve apt-get install -y --no-install-recommends curl.",
    ),
    RuleText::new(
        "npm install sin versiones fijas",
        "fijá la versión con paquete@versión",
        UNPINNED_WHY,
        "npm install -g pnpm se vuelve npm install -g pnpm@9.12.0.",
    ),
    RuleText::new(
        "apk add sin versiones fijas",
        "fijá cada paquete con paquete=versión",
        UNPINNED_WHY,
        "apk add curl se vuelve apk add curl=8.11.0-r2.",
    ),
    RuleText::new(
        "apk add sin --no-cache",
        "usá apk add --no-cache",
        "Sin --no-cache el índice de paquetes queda en la imagen, o hace falta un update y un rm a mano.",
        "apk update && apk add curl se vuelve apk add --no-cache curl.",
    ),
    RuleText::new(
        "ADD para copiar archivos o carpetas",
        "usá COPY; ADD sólo para extraer un tar o bajar una URL",
        "ADD extrae comprimidos y baja URLs por su cuenta: con archivos comunes hace lo mismo que COPY, pero con sorpresas.",
        "ADD ./app /app se vuelve COPY ./app /app.",
    ),
    RuleText::new(
        "COPY de varios archivos a un destino que no termina en /",
        "terminá el destino con /",
        "Con varios orígenes el destino tiene que ser una carpeta; sin la barra el build falla.",
        "COPY a.txt b.txt /dest se vuelve COPY a.txt b.txt /dest/.",
    ),
    RuleText::new(
        "COPY --from a una etapa que no está definida",
        "apuntá a un alias definido antes con FROM ... AS",
        "Si el nombre no es una etapa anterior, Docker lo busca como imagen y copia de algo que nadie definió.",
        "FROM golang:1.23 AS build ... COPY --from=build /out/app /app.",
    ),
    RuleText::new(
        "COPY --from a la misma etapa",
        "copiá desde otra etapa",
        "Una etapa no puede copiar de sí misma: el build falla.",
        "COPY --from=build va en la etapa final, no dentro de build.",
    ),
    RuleText::new(
        "Dos etapas con el mismo alias",
        "dale a cada etapa un nombre único",
        "Con nombres repetidos, COPY --from apunta a una etapa distinta de la que creés.",
        "FROM node:22 AS build y FROM golang:1.23 AS build se vuelven build-web y build-api.",
    ),
    RuleText::new(
        "CMD o ENTRYPOINT en forma de shell",
        "usá la forma JSON: CMD [\"programa\", \"argumento\"]",
        "En forma de shell el proceso corre debajo de /bin/sh -c: no recibe SIGTERM y el contenedor no se apaga limpio.",
        "CMD node server.js se vuelve CMD [\"node\", \"server.js\"].",
    ),
    RuleText::new(
        "apt en vez de apt-get",
        "usá apt-get o apt-cache",
        "apt está pensado para personas: su salida y su interfaz cambian entre versiones y no son estables para un script.",
        "apt install -y curl se vuelve apt-get install -y curl.",
    ),
    RuleText::new(
        "FROM con un --platform fijo",
        "sacá --platform o usá $BUILDPLATFORM o $TARGETPLATFORM",
        "Un --platform fijo impide armar la imagen para otras arquitecturas y emula de más en cada build.",
        "FROM --platform=linux/amd64 node:22 se vuelve FROM --platform=$BUILDPLATFORM node:22.",
    ),
    RuleText::new(
        "yum install sin yum clean all después",
        "terminá el RUN con yum clean all",
        "La caché de yum queda en la capa y suma un peso que ningún RUN posterior saca.",
        "RUN yum install -y curl && yum clean all",
    ),
    RuleText::new(
        "pip install guarda su caché en la imagen",
        "agregá --no-cache-dir o ENV PIP_NO_CACHE_DIR=1",
        "La caché de pip sólo sirve para el próximo install en la misma máquina: en la imagen es peso muerto.",
        "pip install -r requirements.txt se vuelve pip install --no-cache-dir -r requirements.txt.",
    ),
    RuleText::new(
        "COPY a un destino relativo sin WORKDIR",
        "definí WORKDIR antes o usá un destino absoluto",
        "Sin WORKDIR, la ruta relativa depende de la imagen base y puede terminar en /.",
        "COPY . app se vuelve WORKDIR /app y COPY . .",
    ),
    RuleText::new(
        "wget sin controlar la barra de progreso",
        "usá wget --progress=dot:giga, -q o -nv",
        "La barra de progreso de wget llena el log del build con miles de líneas.",
        "wget https://x/archivo.tar.gz se vuelve wget -q https://x/archivo.tar.gz.",
    ),
    RuleText::new(
        "Varios RUN seguidos que se pueden unir",
        "uní los RUN consecutivos con &&",
        "Cada RUN es una capa más: más peso, más tiempo y una caché que se invalida de a pedazos.",
        "RUN apt-get update y RUN apt-get install -y curl se vuelven RUN apt-get update && apt-get install -y curl.",
    ),
    RuleText::new(
        "Instrucción antes del primer FROM",
        "empezá el Dockerfile con FROM o ARG",
        "Antes del primer FROM sólo valen ARG y comentarios: lo demás no pertenece a ninguna etapa y el build falla.",
        "Mové ese LABEL o ENV debajo del FROM.",
    ),
    RuleText::new(
        "go install o go run sin una versión fija",
        "fijá la versión con paquete@vX.Y.Z",
        UNPINNED_WHY,
        "go install golang.org/x/tools/gopls@latest se vuelve go install golang.org/x/tools/gopls@v0.16.2.",
    ),
    RuleText::new(
        "ARG o ENV con el nombre de un dato sensible",
        "pasá los secretos con RUN --mount=type=secret, no con ARG ni ENV",
        "ARG y ENV quedan en el historial de la imagen: cualquiera que la baje lee el valor con docker history.",
        "ARG NPM_TOKEN se vuelve RUN --mount=type=secret,id=npm_token ...",
    ),
    RuleText::new(
        "USER con un nombre en vez de un UID numérico",
        "usá el UID numérico, y el GID si hace falta, en USER",
        "Kubernetes y otros orquestadores verifican runAsNonRoot con el número: con un nombre no pueden saber si es root.",
        "USER app se vuelve USER 10001:10001.",
    ),
    RuleText::new(
        "COPY --from copia el sistema de archivos entero",
        "copiá sólo las carpetas o archivos que hacen falta",
        "Copiar / de otra etapa trae binarios, cachés y archivos de sistema que pisan los de la etapa actual.",
        "COPY --from=build / / se vuelve COPY --from=build /out/app /usr/local/bin/app.",
    ),
    RuleText::new(
        "MAINTAINER está deprecado",
        "usá LABEL maintainer=...",
        "MAINTAINER ya no forma parte de la especificación: lo reemplaza una etiqueta que leen las herramientas.",
        "MAINTAINER Ana <ana@ejemplo.com> se vuelve LABEL maintainer=\"ana@ejemplo.com\".",
    ),
    RuleText::new(
        "wget y curl en la misma etapa",
        "usá una sola de las dos herramientas",
        "Instalar dos herramientas para lo mismo suma peso y dos comportamientos distintos ante errores.",
        "Si ya usás curl, wget -O x URL se vuelve curl -fsSLo x URL.",
    ),
    RuleText::new(
        "Más de un CMD en la misma etapa",
        "dejá un solo CMD",
        "Sólo vale el último CMD: los anteriores no corren nunca y confunden.",
        "Borrá los CMD de más.",
    ),
    RuleText::new(
        "Más de un ENTRYPOINT en la misma etapa",
        "dejá un solo ENTRYPOINT",
        "Sólo vale el último ENTRYPOINT: los anteriores no corren nunca y confunden.",
        "Borrá los ENTRYPOINT de más.",
    ),
    RuleText::new(
        "RUN con un pipe sin pipefail",
        "poné SHELL [\"/bin/bash\", \"-o\", \"pipefail\", \"-c\"] antes del RUN",
        "Sin pipefail, en curl ... | sh sólo cuenta el último comando: si la descarga falla, el build sigue como si nada.",
        "SHELL [\"/bin/bash\", \"-o\", \"pipefail\", \"-c\"] y después RUN curl -fsSL URL | sh.",
    ),
    RuleText::new(
        "Un comando que no tiene sentido dentro de un contenedor (ssh, vim, service, ps, top, kill, mount, free, shutdown)",
        "sacalo del Dockerfile",
        "En un build no hay servicios ni terminal: el comando no hace lo que parece y suele esconder un error de diseño de la imagen.",
        "RUN service nginx start se reemplaza por un CMD que arranque nginx.",
    ),
    RuleText::new(
        "Copia un archivo comprimido y lo descomprime en un RUN aparte",
        "usá ADD, que lo descomprime al copiarlo",
        "El archivo comprimido queda en una capa y lo descomprimido en otra: la imagen pesa las dos cosas.",
        "COPY app.tar.gz /tmp/ y RUN tar -xzf /tmp/app.tar.gz se vuelven ADD app.tar.gz /app/.",
    ),
    RuleText::new(
        "EXPOSE con un puerto fuera de rango",
        "usá un puerto entre 0 y 65535",
        "Un puerto mayor a 65535 no existe: Docker rechaza la imagen o el puerto no se publica.",
        "EXPOSE 80000 se vuelve EXPOSE 8000.",
    ),
    RuleText::new(
        "gem install sin versión fija",
        "fijá la versión: gem install <gema>:<versión>",
        "Sin versión, cada build instala lo último que haya publicado: dos builds del mismo commit no dan la misma imagen.",
        "gem install bundler se vuelve gem install bundler:2.5.6.",
    ),
    RuleText::new(
        "yum install sin -y",
        "agregá -y",
        "Sin -y, yum pregunta y el build espera una respuesta que nunca llega.",
        "yum install httpd se vuelve yum install -y httpd.",
    ),
    RuleText::new(
        "yum install sin versión fija",
        "fijá la versión: yum install -y <paquete>-<versión>",
        "Sin versión, cada build instala lo último que haya publicado: dos builds del mismo commit no dan la misma imagen.",
        "yum install -y httpd se vuelve yum install -y httpd-2.4.57.",
    ),
    RuleText::new(
        "zypper sin el modo no interactivo",
        "agregá -y (o --non-interactive)",
        "Sin el modo no interactivo, zypper pregunta y el build espera una respuesta que nunca llega.",
        "zypper install curl se vuelve zypper install -y curl.",
    ),
    RuleText::new(
        "zypper dist-upgrade en un Dockerfile",
        "no actualices la distribución entera: partí de una imagen base más nueva",
        "dist-upgrade cambia la distribución entera en cada build: la imagen deja de ser reproducible y puede romperse sola.",
        "RUN zypper dist-upgrade se reemplaza por un FROM con una versión más nueva.",
    ),
    RuleText::new(
        "zypper install sin zypper clean después",
        "agregá zypper clean en el mismo RUN",
        "La caché de zypper queda en la capa y engorda la imagen sin servir para nada.",
        "RUN zypper install -y curl se vuelve RUN zypper install -y curl && zypper clean.",
    ),
    RuleText::new(
        "zypper install sin versión fija",
        "fijá la versión: zypper install -y <paquete>=<versión>",
        "Sin versión, cada build instala lo último que haya publicado: dos builds del mismo commit no dan la misma imagen.",
        "zypper install -y curl se vuelve zypper install -y curl=8.6.0.",
    ),
    RuleText::new(
        "dnf install sin -y",
        "agregá -y",
        "Sin -y, dnf pregunta y el build espera una respuesta que nunca llega.",
        "dnf install httpd se vuelve dnf install -y httpd.",
    ),
    RuleText::new(
        "dnf install sin dnf clean all después",
        "agregá dnf clean all en el mismo RUN",
        "La caché de dnf queda en la capa y engorda la imagen sin servir para nada.",
        "RUN dnf install -y httpd se vuelve RUN dnf install -y httpd && dnf clean all.",
    ),
    RuleText::new(
        "dnf install sin versión fija",
        "fijá la versión: dnf install -y <paquete>-<versión>",
        "Sin versión, cada build instala lo último que haya publicado: dos builds del mismo commit no dan la misma imagen.",
        "dnf install -y httpd se vuelve dnf install -y httpd-2.4.57.",
    ),
    RuleText::new(
        "ONBUILD con ONBUILD, FROM o MAINTAINER adentro",
        "sacá esa instrucción del ONBUILD",
        "Docker no permite encadenar ONBUILD ni disparar FROM o MAINTAINER desde uno: el build de la imagen hija falla.",
        "ONBUILD FROM base se borra.",
    ),
    RuleText::new(
        "Un ENV usa una variable que define en la misma instrucción",
        "definí la variable en un ENV anterior",
        "Dentro del mismo ENV la variable todavía no existe: se reemplaza por vacío o por el valor viejo, no por el nuevo.",
        "ENV A=1 B=$A se vuelve ENV A=1 y después ENV B=$A.",
    ),
    RuleText::new(
        "useradd con un UID alto sin -l",
        "agregá -l (--no-log-init)",
        "Sin -l, useradd reserva espacio en /var/log/lastlog según el UID: con un UID grande la imagen engorda gigas.",
        "useradd -u 1000000 app se vuelve useradd -l -u 1000000 app.",
    ),
    RuleText::new(
        "yarn install sin yarn cache clean después",
        "agregá yarn cache clean en el mismo RUN, o usá un mount de caché",
        "La caché de yarn queda en la capa y engorda la imagen sin servir para nada.",
        "RUN yarn install se vuelve RUN yarn install && yarn cache clean.",
    ),
    RuleText::new(
        "Una etapa se llama scratch o context",
        "usá otro nombre para la etapa",
        "scratch y context son nombres reservados: Docker los interpreta como la imagen vacía o el contexto del build, no como la etapa.",
        "FROM node AS scratch se vuelve FROM node AS build.",
    ),
    RuleText::new(
        "FROM --platform=$TARGETPLATFORM de más",
        "sacá el --platform",
        "Es el valor por defecto: la bandera no cambia nada y hace pensar que la etapa se arma para otra plataforma.",
        "FROM --platform=$TARGETPLATFORM node se vuelve FROM node.",
    ),
    RuleText::new(
        "ln sobre /bin/sh para cambiar de shell",
        "usá la instrucción SHELL",
        "Reemplazar /bin/sh cambia el shell de todo lo que corre en la imagen, no sólo del build.",
        "RUN ln -sf /bin/bash /bin/sh se vuelve SHELL [\"/bin/bash\", \"-c\"].",
    ),
    RuleText::new(
        "Una clave de LABEL inválida",
        "usá minúsculas, números, '.', '-', '_' y '/', sin separadores dobles ni los espacios reservados de Docker",
        "Las herramientas que leen los labels esperan claves en ese formato; com.docker., io.docker. y org.dockerproject. están reservados para Docker.",
        "LABEL Version=1 se vuelve LABEL org.opencontainers.image.version=1.",
    ),
];

pub fn dockerfile(rule: DockerfileRule) -> RuleText {
    DOCKERFILE_TEXTS
        .get(rule.position())
        .copied()
        .unwrap_or(UNKNOWN_DOCKERFILE)
}
