typeof valor === "strng";
typeof valor == "string";
"nmber" !== typeof valor;
typeof valor === `object`;
typeof valor === `obj${"ect"}`;
typeof valor === 1;
typeof valor === null;
typeof valor === undefined;
typeof valor === otro;
(typeof valor) === "fnction";
typeof valor === typeof otro;
typeof valor < "x";

const {} = objeto;
const [] = lista;
const [,] = lista;
const { a: {} } = objeto;
function vacia({}, []) {}
for (const {} of lista) {}

const huecos = [1, , 3];
const inicio = [, 1];
const doble = [1, , , 2];
const final = [1, ,];
const comun = [1, 2,];
const vacio = [,];

function* sinYield() {
  return 1;
}
function* vacioGenerador() {}
function* conYield() {
  yield 1;
}
function* anidado() {
  function* interno() {
    yield 2;
  }
  return interno;
}
const objetoGenerador = {
  *metodo() {
    return 3;
  },
  propiedad: function* () {
    return 4;
  },
};
class ClaseGeneradora {
  *metodo() {
    return 5;
  }
  static *estatico() {
    yield 6;
  }
}

try {
  cargar();
} catch (error) {
  throw error;
}
try {
  cargar();
} catch (error) {
  throw error;
} finally {
  cerrar();
}
try {
  cargar();
} catch (error) {
  registrar(error);
  throw error;
}
try {
  cargar();
} catch ({ message }) {
  throw message;
}
try {
  cargar();
} catch (error) {
  throw otro;
}

valor === -0;
-0 <= valor;
valor === -0.0;
valor === 0;
valor === -1;
valor === -0n;
valor + -0;

valor === NaN;
NaN != valor;
valor < Number.NaN;
valor === Number["NaN"];
valor === (otro, NaN);
switch (NaN) {
  case NaN:
    break;
  default:
}
switch (valor) {
  case Number.NaN:
    break;
}
lista.indexOf(NaN);
function sombraNaN(NaN, Number) {
  return valor === NaN || valor === Number.NaN;
}

new Promise(async (resolver) => resolver());
new Promise(async function (resolver) {
  resolver();
});
new Promise((resolver) => resolver());
new Promesa(async () => {});
function sombraPromise(Promise) {
  return new Promise(async () => {});
}

afuera: for (const item of lista) {
  for (const otro of item) {
    continue afuera;
  }
}
sinUso: for (const item of lista) {
  break;
}
bloque: {
  registrar(1);
}
usado: {
  break usado;
}

let propio = 1;
propio = propio;
propio &&= propio;
propio += propio;
[propio, otro] = [propio, otro];
[propio, ...resto] = [propio, ...resto];
[propio, ...resto] = [otro, propio, ...resto];
({ propio } = { propio });
({ propio: alias } = { propio: alias });
({ propio } = { ...otro, propio });
({ propio } = { propio, ...otro });
objeto.campo = objeto.campo;
objeto.campo = objeto["campo"];
objeto[0] = objeto[0.0];
objeto[clave] = objeto[clave];
objeto.a.b = objeto.a.c;
this.campo = this.campo;
(objeto).campo = objeto.campo;
