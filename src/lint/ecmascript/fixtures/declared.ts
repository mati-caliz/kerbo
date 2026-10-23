declare let ambiente: number;
declare namespace Espacio {
  let interno: number;
}
let tipada: number;
console.log(ambiente, Espacio, tipada);
type Medida = typeof tipada;

const fija = 1;
fija = 2;
class Clase {}
Clase = null;

function firma(NaN: number): void;
function firma(valor: number): void {}
type Funcion = (undefined: number) => void;
