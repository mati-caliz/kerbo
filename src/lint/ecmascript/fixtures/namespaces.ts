namespace Utilidades {
  export const valor = 1;
  namespace Interna {}
}
declare namespace Declarado {}
module Viejo {}
declare module "paquete" {}
declare global {
  interface Ventana {}
}
export namespace Exportado {}
namespace A.B.C {}

const cadena = usuario?.nombre!;
const sinOpcional = usuario.nombre!;
const larga = usuario?.datos!.nombre;
const llamada = usuario?.cargar()!;
const tipado = new Array<string>();
const arreglos = Array<number>(1, 2);
class Accesor {
  set valor(dato: number) {
    return;
  }
}
