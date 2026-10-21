function blocks(value) {
  if (value) {
  } else {
    // comentado: no cuenta como vacío
  }
  try {
  } catch (error) {
  } finally {
  }
  switch (value) {
  }
  switch (value) {
    // tampoco
  }
  label: {
  }
  const arrow = () => {};
  class Holder {
    static {}
    method() {}
  }
  debugger;
  if (value) debugger;
  return [arrow, Holder];
}

function cases(value) {
  switch (value) {
    case 1:
      let first = 1;
      break;
    case 2:
      const second = 2;
      function third() {}
      class Fourth {}
      var fifth = 5;
      break;
    default:
      let sixth;
      {
        let seventh;
      }
  }
  switch (value) {
    case 1:
    case 2:
    case 1:
      break;
    case "1":
    case (2):
    case a + b:
    case a+b:
    case (a) + b:
    case f(x):
    case f( x ):
  }
}

function conditions(a, b) {
  if (a = b) {}
  if ((a = b)) {}
  if (((a = b))) {}
  while (a += 1) {}
  do {} while (a = b);
  do {} while ((a = b));
  for (; a = b; ) {}
  for (; (a = b); ) {}
  const value = (a = b) ? 1 : 2;
  const other = ((a = b)) ? 1 : 2;
  if (a === b && (a = b)) {}
  while ((a = b) !== null) {}
  return [value, other];
}

function builtins(object, key) {
  object.hasOwnProperty(key);
  object?.isPrototypeOf(key);
  object.propertyIsEnumerable?.(key);
  object["hasOwnProperty"](key);
  object[`isPrototypeOf`](key);
  (object.hasOwnProperty)(key);
  Object.prototype.hasOwnProperty.call(object, key);
  object.hasOwnProperty`key`;
  object.has(key);
}

function chains(a, b, c) {
  if (a) {} else if (a) {}
  if (a || b) {} else if (a) {}
  if (a) {} else if (a || b) {}
  if (a && b) {} else if (b && a) {}
  if (a) {} else if (b) {} else if (a && c) {}
  if ((a)) {} else if (a) {}
  if (a) {} else { if (a) {} }
  if (a || b) {} else if (b || a) {}
  if (a) {} else if (b) {} else if (c) {} else if (b || c) {}
}

const keys = {
  a: 1,
  "a": 2,
  b: 1,
  get b() { return 1; },
  get c() { return 1; },
  set c(value) {},
  set c(value) {},
  1: 1,
  "1": 2,
  0x1: 3,
  1.0: 4,
  ["d"]: 1,
  [`d`]: 2,
  d() {},
  __proto__: null,
  __proto__: null,
  ["__proto__"]: 1,
  ["__proto__"]: 2,
  e,
  e,
  ...spread,
  ...spread,
  [computed]: 1,
  [computed]: 2,
  1e3: 1,
  1000: 2,
  get: 1,
  get() {},
};

// @ts-ignore
const ignored = 1;
/* @ts-expect-error */
const expected = 2;
// @ts-expect-error: con motivo
const described = 3;
// @ts-nocheck
export { blocks, cases, conditions, builtins, chains, keys, ignored, expected, described, trailing };

function trailing(value) {
  if (value) {
  } // el comentario de después no cuenta
  switch (value) {
  } // tampoco acá
}
