if (1) {}
if ("") {}
if (`texto`) {}
if (`${valor}`) {}
if (`\
${valor}`) {}
if ([]) {}
if ({}) {}
if (function () {}) {}
if (() => 1) {}
if (class {}) {}
if (new Fecha()) {}
if (void valor) {}
if (typeof valor) {}
if (!valor) {}
if (!1) {}
if (-1) {}
if (1 + 2) {}
if ("a" in objeto) {}
if (valor || true) {}
if (true || valor) {}
if (valor && false) {}
if (false && valor) {}
if (valor ?? 1) {}
if ((valor, 1)) {}
if ((valor = 1)) {}
if ((valor ||= true)) {}
if ((valor &&= 0)) {}
if ((valor += 1)) {}
if (Boolean(1)) {}
if (Boolean()) {}
if (Boolean?.(1)) {}
if (Boolean(valor)) {}
if (undefined) {}
if (/regex/) {}
if (null) {}
if (0n) {}
if (0x0 || valor) {}
if (1e-400 || valor) {}
const resultado = 1 ? a : b;
const anidado = (valor ? 2 : 3) ? a : b;

while (true) {}
while ((true)) {}
while (1) {}
do {} while (1);
for (;;) {}
for (; 1;) {}
for (let indice = 0; true; indice++) {}

function* generador() {
  while (1) {
    yield 1;
  }
  for (yield 2; true;) {}
  while (1) {
    function interna() {}
    interna();
  }
  while (1) {
    function* otro() {
      yield 3;
    }
    otro();
  }
  while (1) {
    while (2) {}
    yield 4;
  }
}

function sombra(undefined, Boolean) {
  if (undefined) {}
  if (Boolean(1)) {}
}
