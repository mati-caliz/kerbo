function usesDeletes(a) {
  return a;
}
const octalNumber = 1;
const decimalEscapes = "a";
if (!octalNumber in decimalEscapes) usesDeletes(1);
if ((!octalNumber) in decimalEscapes) usesDeletes(2);
if (!(octalNumber in decimalEscapes)) usesDeletes(3);
if (!octalNumber instanceof Object) usesDeletes(4);
if (!octalNumber < 1) usesDeletes(5);
class Blocks {
  static {}
  static { /* comentario */ }
  static { usesDeletes(6); }
  static {
    // comentario
  }
}
new Symbol("a");
new BigInt(1);
Symbol("a");
function shadows(Symbol, Math) {
  new Symbol("b");
  Math();
}
Math();
new Math();
JSON();
Reflect();
Atomics();
Intl();
globalThis.Math();
new globalThis.JSON();
Math.floor(1);
globalThis();
function duplicates(a, b, [b2], {c, d: e}, f = 1, ...g) { return [a, b, b2, c, e, f, g]; }
const expression = function (x, y) { return [x, y]; };
class Parameters { method(z) { return z; } }
const arrow = (p, q) => [p, q];
class Members {
  a() {}
  a() {}
  static a() {}
  static a() {}
  get b() { return 1; }
  set b(value) { usesDeletes(value); }
  get b() { return 2; }
  c = 1;
  c = 2;
  d() {}
  get d() { return 1; }
  'e'() {}
  e() {}
  ['f']() {}
  f() {}
  [expression]() {}
  [expression]() {}
  constructor() {}
  static constructor() {}
  static constructor() {}
}
for (let i = 0; i < 10; i--) {}
for (let i = 0; i < 10; i++) {}
for (let i = 10; i > 0; i++) {}
for (let i = 10; i > 0; i--) {}
for (let i = 0; 10 > i; i--) {}
for (let i = 0; 10 < i; i++) {}
for (let i = 0; i <= 10; i -= 1) {}
for (let i = 0; i <= 10; i += -1) {}
for (let i = 0; i <= 10; i += 1) {}
for (let i = 10; i >= 0; i -= -1) {}
for (let i = 0; i < 10; i += expression) {}
for (let i = 0, j = 0; i < 10; i--, j++) {}
for (let i = 0; i < 10; i--, i++) {}
for (let i = 0; i !== 10; i--) {}
for (let i = 0; i < 10; ) {}
for (let i = 0; i < 10; (i--)) {}
try {
  usesDeletes(7);
} catch (error) {
  error = 1;
  error += 1;
  error++;
  usesDeletes(error);
}
try {
  usesDeletes(8);
} catch ({ message }) {
  message = 1;
}
try {
  usesDeletes(9);
} catch {
  usesDeletes(10);
}
export { arrow, shadows, duplicates, Blocks, Parameters, Members };
