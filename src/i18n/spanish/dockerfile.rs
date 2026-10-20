use crate::i18n::catalog::RuleText;
use crate::rules::DockerfileRule;

const UNPINNED_WHY: &str = "Sin versión, cada build instala lo último que haya publicado: dos builds del mismo commit no dan la misma imagen.";
const LATEST_WHY: &str =
    "Sin una versión fija, el mismo Dockerfile arma otra imagen el día que publican una base nueva.";

pub const fn dockerfile(rule: DockerfileRule) -> RuleText {
    match rule {
        DockerfileRule::Dl3000 => RuleText::new(
            "WORKDIR con una ruta relativa",
            "usá una ruta absoluta en WORKDIR",
            "Una ruta relativa depende del WORKDIR anterior y de la imagen base: cambia sin que nadie toque este Dockerfile.",
            "WORKDIR app se vuelve WORKDIR /app.",
        ),
        DockerfileRule::Dl3002 => RuleText::new(
            "La etapa termina corriendo como root",
            "terminá la etapa con un USER sin privilegios",
            "Si el proceso corre como root, cualquier vulnerabilidad en él es root en el contenedor y un paso más cerca del host.",
            "Después de instalar, RUN useradd -r app y USER app.",
        ),
        DockerfileRule::Dl3003 => RuleText::new(
            "cd dentro de un RUN",
            "usá WORKDIR para cambiar de carpeta",
            "Un cd vale sólo para ese RUN: el siguiente arranca en otro lado y es fácil olvidarlo.",
            "RUN cd /app && make se vuelve WORKDIR /app y RUN make.",
        ),
        DockerfileRule::Dl3004 => RuleText::new(
            "sudo dentro de un RUN",
            "corré el paso como root con USER, o usá gosu en el entrypoint",
            "Cada RUN ya corre como el USER actual; sudo suma un binario con setuid y un comportamiento que depende de su configuración.",
            "RUN sudo apt-get install ... se vuelve USER root, el RUN, y otra vez USER app.",
        ),
        DockerfileRule::Dl3006 => RuleText::new(
            "Imagen base sin etiqueta",
            "fijá la versión de la imagen base",
            LATEST_WHY,
            "FROM python se vuelve FROM python:3.12-slim.",
        ),
        DockerfileRule::Dl3007 => RuleText::new(
            "Imagen base con la etiqueta latest",
            "fijá una versión concreta o un digest",
            LATEST_WHY,
            "FROM node:latest se vuelve FROM node:22.11-alpine.",
        ),
        DockerfileRule::Dl3008 => RuleText::new(
            "apt-get install sin versiones fijas",
            "fijá cada paquete con paquete=versión",
            UNPINNED_WHY,
            "apt-get install -y curl se vuelve apt-get install -y curl=8.5.0-2.",
        ),
        DockerfileRule::Dl3009 => RuleText::new(
            "Quedan las listas de apt en la imagen",
            "borrá /var/lib/apt/lists en el mismo RUN que hace apt-get update",
            "Las listas pesan decenas de MB y, si quedan en una capa, ningún RUN posterior las saca de la imagen.",
            "RUN apt-get update && apt-get install -y curl && rm -rf /var/lib/apt/lists/*",
        ),
        DockerfileRule::Dl3012 => RuleText::new(
            "Más de un HEALTHCHECK en la misma etapa",
            "dejá un solo HEALTHCHECK",
            "Sólo vale el último: los anteriores confunden a quien lee y no chequean nada.",
            "Borrá los HEALTHCHECK de más y quedate con el que refleja el servicio.",
        ),
        DockerfileRule::Dl3013 => RuleText::new(
            "pip install sin versiones fijas",
            "fijá las versiones con paquete==versión o instalá desde un requirements con versiones",
            UNPINNED_WHY,
            "pip install requests se vuelve pip install requests==2.32.3.",
        ),
        DockerfileRule::Dl3014 => RuleText::new(
            "apt-get install sin -y",
            "agregá -y para que no pida confirmación",
            "Sin -y el build queda esperando una respuesta que nunca llega y falla.",
            "apt-get install curl se vuelve apt-get install -y curl.",
        ),
        DockerfileRule::Dl3015 => RuleText::new(
            "apt-get install trae los paquetes recomendados",
            "agregá --no-install-recommends",
            "Los recomendados suman paquetes que nadie pidió: más peso, más superficie de ataque y más tiempo de build.",
            "apt-get install -y curl se vuelve apt-get install -y --no-install-recommends curl.",
        ),
        DockerfileRule::Dl3016 => RuleText::new(
            "npm install sin versiones fijas",
            "fijá la versión con paquete@versión",
            UNPINNED_WHY,
            "npm install -g pnpm se vuelve npm install -g pnpm@9.12.0.",
        ),
        DockerfileRule::Dl3018 => RuleText::new(
            "apk add sin versiones fijas",
            "fijá cada paquete con paquete=versión",
            UNPINNED_WHY,
            "apk add curl se vuelve apk add curl=8.11.0-r2.",
        ),
        DockerfileRule::Dl3019 => RuleText::new(
            "apk add sin --no-cache",
            "usá apk add --no-cache",
            "Sin --no-cache el índice de paquetes queda en la imagen, o hace falta un update y un rm a mano.",
            "apk update && apk add curl se vuelve apk add --no-cache curl.",
        ),
        DockerfileRule::Dl3020 => RuleText::new(
            "ADD para copiar archivos o carpetas",
            "usá COPY; ADD sólo para extraer un tar o bajar una URL",
            "ADD extrae comprimidos y baja URLs por su cuenta: con archivos comunes hace lo mismo que COPY, pero con sorpresas.",
            "ADD ./app /app se vuelve COPY ./app /app.",
        ),
        DockerfileRule::Dl3021 => RuleText::new(
            "COPY de varios archivos a un destino que no termina en /",
            "terminá el destino con /",
            "Con varios orígenes el destino tiene que ser una carpeta; sin la barra el build falla.",
            "COPY a.txt b.txt /dest se vuelve COPY a.txt b.txt /dest/.",
        ),
        DockerfileRule::Dl3022 => RuleText::new(
            "COPY --from a una etapa que no está definida",
            "apuntá a un alias definido antes con FROM ... AS",
            "Si el nombre no es una etapa anterior, Docker lo busca como imagen y copia de algo que nadie definió.",
            "FROM golang:1.23 AS build ... COPY --from=build /out/app /app.",
        ),
        DockerfileRule::Dl3023 => RuleText::new(
            "COPY --from a la misma etapa",
            "copiá desde otra etapa",
            "Una etapa no puede copiar de sí misma: el build falla.",
            "COPY --from=build va en la etapa final, no dentro de build.",
        ),
        DockerfileRule::Dl3024 => RuleText::new(
            "Dos etapas con el mismo alias",
            "dale a cada etapa un nombre único",
            "Con nombres repetidos, COPY --from apunta a una etapa distinta de la que creés.",
            "FROM node:22 AS build y FROM golang:1.23 AS build se vuelven build-web y build-api.",
        ),
        DockerfileRule::Dl3025 => RuleText::new(
            "CMD o ENTRYPOINT en forma de shell",
            "usá la forma JSON: CMD [\"programa\", \"argumento\"]",
            "En forma de shell el proceso corre debajo de /bin/sh -c: no recibe SIGTERM y el contenedor no se apaga limpio.",
            "CMD node server.js se vuelve CMD [\"node\", \"server.js\"].",
        ),
        DockerfileRule::Dl3027 => RuleText::new(
            "apt en vez de apt-get",
            "usá apt-get o apt-cache",
            "apt está pensado para personas: su salida y su interfaz cambian entre versiones y no son estables para un script.",
            "apt install -y curl se vuelve apt-get install -y curl.",
        ),
        DockerfileRule::Dl3029 => RuleText::new(
            "FROM con un --platform fijo",
            "sacá --platform o usá $BUILDPLATFORM o $TARGETPLATFORM",
            "Un --platform fijo impide armar la imagen para otras arquitecturas y emula de más en cada build.",
            "FROM --platform=linux/amd64 node:22 se vuelve FROM --platform=$BUILDPLATFORM node:22.",
        ),
        DockerfileRule::Dl3032 => RuleText::new(
            "yum install sin yum clean all después",
            "terminá el RUN con yum clean all",
            "La caché de yum queda en la capa y suma un peso que ningún RUN posterior saca.",
            "RUN yum install -y curl && yum clean all",
        ),
        DockerfileRule::Dl3042 => RuleText::new(
            "pip install guarda su caché en la imagen",
            "agregá --no-cache-dir o ENV PIP_NO_CACHE_DIR=1",
            "La caché de pip sólo sirve para el próximo install en la misma máquina: en la imagen es peso muerto.",
            "pip install -r requirements.txt se vuelve pip install --no-cache-dir -r requirements.txt.",
        ),
        DockerfileRule::Dl3045 => RuleText::new(
            "COPY a un destino relativo sin WORKDIR",
            "definí WORKDIR antes o usá un destino absoluto",
            "Sin WORKDIR, la ruta relativa depende de la imagen base y puede terminar en /.",
            "COPY . app se vuelve WORKDIR /app y COPY . .",
        ),
        DockerfileRule::Dl3047 => RuleText::new(
            "wget sin controlar la barra de progreso",
            "usá wget --progress=dot:giga, -q o -nv",
            "La barra de progreso de wget llena el log del build con miles de líneas.",
            "wget https://x/archivo.tar.gz se vuelve wget -q https://x/archivo.tar.gz.",
        ),
        DockerfileRule::Dl3059 => RuleText::new(
            "Varios RUN seguidos que se pueden unir",
            "uní los RUN consecutivos con &&",
            "Cada RUN es una capa más: más peso, más tiempo y una caché que se invalida de a pedazos.",
            "RUN apt-get update y RUN apt-get install -y curl se vuelven RUN apt-get update && apt-get install -y curl.",
        ),
        DockerfileRule::Dl3061 => RuleText::new(
            "Instrucción antes del primer FROM",
            "empezá el Dockerfile con FROM o ARG",
            "Antes del primer FROM sólo valen ARG y comentarios: lo demás no pertenece a ninguna etapa y el build falla.",
            "Mové ese LABEL o ENV debajo del FROM.",
        ),
        DockerfileRule::Dl3062 => RuleText::new(
            "go install o go run sin una versión fija",
            "fijá la versión con paquete@vX.Y.Z",
            UNPINNED_WHY,
            "go install golang.org/x/tools/gopls@latest se vuelve go install golang.org/x/tools/gopls@v0.16.2.",
        ),
        DockerfileRule::Dl3064 => RuleText::new(
            "ARG o ENV con el nombre de un dato sensible",
            "pasá los secretos con RUN --mount=type=secret, no con ARG ni ENV",
            "ARG y ENV quedan en el historial de la imagen: cualquiera que la baje lee el valor con docker history.",
            "ARG NPM_TOKEN se vuelve RUN --mount=type=secret,id=npm_token ...",
        ),
        DockerfileRule::Dl3066 => RuleText::new(
            "USER con un nombre en vez de un UID numérico",
            "usá el UID numérico, y el GID si hace falta, en USER",
            "Kubernetes y otros orquestadores verifican runAsNonRoot con el número: con un nombre no pueden saber si es root.",
            "USER app se vuelve USER 10001:10001.",
        ),
        DockerfileRule::Dl3067 => RuleText::new(
            "COPY --from copia el sistema de archivos entero",
            "copiá sólo las carpetas o archivos que hacen falta",
            "Copiar / de otra etapa trae binarios, cachés y archivos de sistema que pisan los de la etapa actual.",
            "COPY --from=build / / se vuelve COPY --from=build /out/app /usr/local/bin/app.",
        ),
        DockerfileRule::Dl4000 => RuleText::new(
            "MAINTAINER está deprecado",
            "usá LABEL maintainer=...",
            "MAINTAINER ya no forma parte de la especificación: lo reemplaza una etiqueta que leen las herramientas.",
            "MAINTAINER Ana <ana@ejemplo.com> se vuelve LABEL maintainer=\"ana@ejemplo.com\".",
        ),
        DockerfileRule::Dl4001 => RuleText::new(
            "wget y curl en la misma etapa",
            "usá una sola de las dos herramientas",
            "Instalar dos herramientas para lo mismo suma peso y dos comportamientos distintos ante errores.",
            "Si ya usás curl, wget -O x URL se vuelve curl -fsSLo x URL.",
        ),
        DockerfileRule::Dl4003 => RuleText::new(
            "Más de un CMD en la misma etapa",
            "dejá un solo CMD",
            "Sólo vale el último CMD: los anteriores no corren nunca y confunden.",
            "Borrá los CMD de más.",
        ),
        DockerfileRule::Dl4004 => RuleText::new(
            "Más de un ENTRYPOINT en la misma etapa",
            "dejá un solo ENTRYPOINT",
            "Sólo vale el último ENTRYPOINT: los anteriores no corren nunca y confunden.",
            "Borrá los ENTRYPOINT de más.",
        ),
        DockerfileRule::Dl4006 => RuleText::new(
            "RUN con un pipe sin pipefail",
            "poné SHELL [\"/bin/bash\", \"-o\", \"pipefail\", \"-c\"] antes del RUN",
            "Sin pipefail, en curl ... | sh sólo cuenta el último comando: si la descarga falla, el build sigue como si nada.",
            "SHELL [\"/bin/bash\", \"-o\", \"pipefail\", \"-c\"] y después RUN curl -fsSL URL | sh.",
        ),
    }
}
