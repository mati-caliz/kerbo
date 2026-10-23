const objeto = {
  get completo() {
    return 1;
  },
  get vacio() {},
  get condicional() {
    if (bandera) {
      return 1;
    }
  },
  get sinValor() {
    return;
  },
  get lanza() {
    throw new Error("x");
  },
  get anidado() {
    const interna = () => {
      return;
    };
    return interna;
  },
  get() {
    return;
  },
};
class Clase {
  get campo() {
    if (bandera) {
      return 1;
    } else {
      return 2;
    }
  }
  static get estatico() {
    registrar();
  }
  get conSwitch() {
    switch (bandera) {
      case 1:
        return 1;
      default:
        return 2;
    }
  }
}
Object.defineProperty(objeto, "a", {
  get() {
    registrar();
  },
});
Object.defineProperty(objeto, "b", {
  get: function () {
    registrar();
  },
});
Object.defineProperty(objeto, "c", { get: () => {} });
Object.defineProperty(objeto, "d", { get: () => 1 });
Object.defineProperties(objeto, { e: { get() {} } });
otro.defineProperty(objeto, "f", { get() {} });
