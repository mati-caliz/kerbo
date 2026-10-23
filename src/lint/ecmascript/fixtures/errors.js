const objeto = {
  set nombre(valor) {
    return valor;
  },
  set otro(valor) {
    const interna = () => {
      return valor;
    };
    interna();
    return;
  },
  set(valor) {
    return valor;
  },
};
class Clase {
  set campo(valor) {
    if (valor) {
      return 1;
    }
  }
  static set estatico(valor) {
    function anidada() {
      return valor;
    }
    anidada();
  }
}
Object.defineProperty(objeto, "x", {
  set(valor) {
    return valor;
  },
});
Object.defineProperty(objeto, "y", {
  set: function (valor) {
    return valor;
  },
});
Reflect.defineProperty(objeto, "z", { set: (valor) => valor });
Object.create(null, { w: { set: (valor) => valor } });
Object.defineProperties(objeto, { v: { set: (valor) => { return valor; } } });
otro.defineProperty(objeto, "u", { set: (valor) => valor });

(objeto?.metodo)();
(objeto?.metodo).campo;
objeto?.metodo();
objeto?.campo.otro;
new (objeto?.Clase)();
const { campo } = objeto?.valor;
const [primero] = objeto?.lista;
[...(objeto?.lista)];
for (const item of objeto?.lista) {}
"x" in objeto?.valor;
valor instanceof objeto?.Clase;
class Hija extends (objeto?.Clase) {}
(objeto?.a || objeto?.b).c;
(objeto?.a && otro).c;
(objeto?.a ?? otro).c;
(condicion ? objeto?.a : otro).c;
(otro, objeto?.a).c;
async function asincrona() {
  (await objeto?.promesa).campo;
}
function* generadora() {
  yield* objeto?.lista;
}
({ campo } = objeto?.valor);
function porDefecto({ a } = objeto?.valor) {
  return a;
}
const copia = { ...objeto?.valor };

function cargarTodo() {
  try {
    cargar();
  } catch (error) {
    throw new Error("fallo");
  }
  try {
    cargar();
  } catch (error) {
    throw new Error("fallo", { cause: error });
  }
  try {
    cargar();
  } catch (error) {
    throw new TypeError("fallo", { cause: otro });
  }
  try {
    cargar();
  } catch (error) {
    throw new Error("fallo", { cause });
  }
  try {
    cargar();
  } catch (error) {
    throw Error("fallo", opciones);
  }
  try {
    cargar();
  } catch (error) {
    throw new Error(...argumentos);
  }
  try {
    cargar();
  } catch (error) {
    throw new AggregateError([], "fallo", { cause: error });
  }
  try {
    cargar();
  } catch (error) {
    throw new AggregateError([], "fallo");
  }
  try {
    cargar();
  } catch ({ message }) {
    throw new Error(message);
  }
  try {
    cargar();
  } catch {
    throw new Error("sin parámetro");
  }
  try {
    cargar();
  } catch (error) {
    const fallar = () => {
      throw new Error("otra función");
    };
    fallar();
  }
  try {
    cargar();
  } catch (error) {
    if (error) {
      const error = 1;
      throw new Error("tapado", { cause: error });
    }
  }
  try {
    cargar();
  } catch (error) {
    throw new Error("propiedades", { ...extra, cause: error });
  }
  try {
    cargar();
  } catch (error) {
    throw new Error("dos", { cause: otro, cause: error });
  }
  try {
    cargar();
  } catch (error) {
    throw new Propio("propio");
  }
  try {
    cargar();
  } catch (error) {
    throw new Error;
  }
}

if (!!valor) {}
if (Boolean(valor)) {}
while (!!valor) {}
do {} while (Boolean(valor));
for (; !!valor; ) {}
for (!!valor; ; ) {}
const ternario = !!valor ? 1 : 2;
const negado = !!!valor;
const negadoLlamada = !Boolean(valor);
const anidadoLlamada = Boolean(!!valor);
const nuevo = new Boolean(Boolean(valor));
const normal = !!valor;
const llamada = Boolean(valor);
if (!!valor && otro) {}
function sombraBoolean(Boolean) {
  if (Boolean(valor)) {}
}

const vacio = new Array();
const varios = Array(1, 2);
const uno = new Array(3);
const sinParentesis = new Array;
const otroArreglo = new Otro(1, 2);
