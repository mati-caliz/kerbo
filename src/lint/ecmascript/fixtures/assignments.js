import fecha, { formato } from "fechas";
import * as utilidades from "utilidades";

const total = 0;
total = 1;
total += 2;
total++;
[total] = [3];
({ total } = { total: 4 });
for (total of [5]) {}
for (const elemento of [6]) {
  elemento = 7;
}

class Pedido {
  cambiar() {
    Pedido = null;
  }
}
Pedido = 8;
const Anonima = class Interna {
  cambiar() {
    Interna = 9;
  }
};
Anonima.x = 1;

fecha = 10;
formato++;
utilidades = 11;
utilidades.campo = 12;
utilidades["otro"] = 13;
utilidades.campo++;
delete utilidades.campo;
for (utilidades.campo in {}) {}
[utilidades.campo] = [14];
({ clave: utilidades.campo } = {});
Object.assign(utilidades, {});
Object.freeze(utilidades);
Object.keys(utilidades);
Reflect.set(utilidades, "campo", 15);
Reflect.get(utilidades, "campo");
utilidades.metodo();
console.log(fecha, formato, utilidades.campo);

undefined = 16;
NaN++;
Object = {};
Math.max = null;
window = {};

function sombras(undefined, NaN) {
  const Infinity = 1;
  let eval;
  try {} catch (arguments) {}
  return [undefined, NaN, Infinity, eval];
}
var undefined;
function globalThis() {}
class arguments {}

let sinValor;
let leidoNunca;
let asignadoDespues;
var tambienSinValor;
asignadoDespues = 17;
console.log(sinValor, asignadoDespues, tambienSinValor);
for (let indice; ; ) {
  console.log(indice);
}

function local(Object) {
  Object.assign(utilidades, {});
}
