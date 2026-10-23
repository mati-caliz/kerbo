function overwritten() {
  let total = 0;
  total = calcular();
  return total;
}

function neverRead() {
  let valor = 1;
  valor = 2;
  return 3;
}

function readAfter() {
  let valor = 1;
  usar(valor);
  valor = 2;
  usar(valor);
}

function branches(condicion) {
  let valor = 1;
  if (condicion) {
    valor = 2;
  }
  return valor;
}

function bothBranches(condicion) {
  let valor = 1;
  if (condicion) {
    valor = 2;
  } else {
    valor = 3;
  }
  return valor;
}

function loops(lista) {
  let ultimo = null;
  for (const item of lista) {
    ultimo = item;
  }
  return ultimo;
}

function counter() {
  let contador = 0;
  contador++;
  contador++;
  return contador;
}

function selfReference() {
  let valor = 1;
  valor = valor + 1;
  return valor;
}

function closures() {
  let valor = 1;
  const leer = () => valor;
  valor = 2;
  return leer;
}

function tries() {
  let valor = 1;
  try {
    valor = cargar();
  } catch {
    return valor;
  }
  return valor;
}

function destructuring(objeto) {
  let { a, b } = objeto;
  a = 1;
  b = 2;
  return a + b;
}

function unused() {
  let nadie = 1;
  nadie = 2;
}

function afterReturn() {
  let valor = 1;
  return valor;
  valor = 2;
}

let moduloExportado = 1;
moduloExportado = 2;
export { moduloExportado };

export let exportado = 1;
exportado = 2;

let modulo = 1;
modulo = 2;
usar(modulo);

function whileLoop(condicion) {
  let intentos = 0;
  while (condicion()) {
    intentos += 1;
  }
  return intentos;
}

function logical(valor) {
  let resultado = valor;
  resultado ||= 1;
  resultado = 3;
  return resultado;
}

function Vista() {
  let texto = "a";
  texto = cargar();
  return <p>{texto}</p>;
}

function Elementos() {
  let Componente = Primero;
  Componente = Segundo;
  return <Componente />;
}
