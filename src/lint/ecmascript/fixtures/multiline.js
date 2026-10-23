const a = b
(c || d).cargar();
const e = f
[1, 2].forEach(registrar);
const g = h
`template`;
const i = j
/regex/g.test(k);
const l = m(
  n,
);
const o = p
  .q();
const r = s?.
[0];
const t = u
();
const v = (w)
(x);

class Privada {
  #usado = 1;
  #sinUso = 2;
  #soloEscrito = 3;
  #incrementado = 0;
  #sumado = 0;
  #leidoConSuma = 0;
  #destructurado;
  #metodo() {}
  #metodoSinUso() {}
  get #accesor() {
    return 1;
  }
  #enArreglo;

  leer() {
    this.#soloEscrito = 4;
    this.#incrementado++;
    this.#sumado += 1;
    const total = (this.#leidoConSuma += 1);
    ({ valor: this.#destructurado } = objeto);
    [this.#enArreglo] = lista;
    this.#metodo();
    this.#accesor;
    return this.#usado + total;
  }
}

class Afuera {
  #interno = 1;
  metodo() {
    return class Adentro {
      #interno = 2;
      leer(otro) {
        return otro.#interno;
      }
    };
  }
}

const flecha = async () => {
  registrar();
}

(async () => {
  await flecha();
})();
