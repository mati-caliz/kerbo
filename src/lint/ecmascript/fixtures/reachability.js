function afterReturn() {
  return 1;
  registrar(1);
  registrar(2);
}

function afterThrow() {
  throw new Error("x");
  const valor = 1;
  var sinValor;
  var conValor = 2;
  function declarada() {}
  registrar(valor, conValor, sinValor, declarada);
}

function branches(condicion) {
  if (condicion) {
    return 1;
  } else {
    return 2;
  }
  registrar(3);
}

function oneBranch(condicion) {
  if (condicion) {
    return 1;
  }
  registrar(2);
}

function loops(lista) {
  while (true) {
    registrar(lista);
  }
  registrar("nunca");
}

function loopWithBreak(lista) {
  while (true) {
    if (lista) {
      break;
    }
  }
  registrar("sí");
  for (;;) {}
  registrar("nunca");
}

function forOf(lista) {
  for (const item of lista) {
    continue;
    registrar(item);
  }
  do {
    break;
    registrar(1);
  } while (lista);
}

function tries() {
  try {
    return cargar();
  } catch (error) {
    registrar(error);
  }
  registrar("después");
  try {
    return 1;
  } finally {
    cerrar();
  }
  registrar("nunca");
}

function tryThrow() {
  try {
    throw new Error("x");
  } catch {
    return 2;
  }
  registrar("nunca");
}

function labels(lista) {
  afuera: {
    break afuera;
    registrar(1);
  }
  registrar(2);
}

function switches(valor) {
  switch (valor) {
    case 1:
      registrar(1);
    case 2:
      registrar(2);
      break;
    case 3:
    case 4:
      registrar(4);
      // falls through
    case 5:
      registrar(5);
      /* fallthrough */
    case 6: {
      registrar(6);
      // falls through
    }
    case 7:
      return 7;
    case 8:
      throw new Error("x");
    case 9:
      if (valor) {
        break;
      } else {
        return;
      }
    case 10:

    case 11:
      registrar(11);
      // exported fallthrough
    default:
      registrar("default");
    case 12:
      registrar(12);
  }
  switch (valor) {
    default:
      registrar(0);
    case 1:
      break;
  }
  switch (valor) {
    case 1:
      while (true) {}
    case 2:
      for (const x of valor) {
        break;
      }
    case 3:
      registrar(3);
  }
}

class Base {}
class Derivada extends Base {
  campo = 1;
  static estatico = 2;
  constructor() {
    return;
  }
}
class ConSuper extends Base {
  campo = 1;
  constructor() {
    super();
  }
}

const flecha = () => {
  return 1;
  registrar(2);
};

function generador() {
  return;
  export1();
}
registrar("programa");
throw new Error("fin");
registrar("después del throw del programa");
