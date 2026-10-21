let never = 1;
let reassigned = 1;
reassigned = 2;
let later;
later = 3;
let both = 1, other = 2;
let partly = 1, changed = 2;
changed = 3;
let { first, second } = source();
let { kept, moved } = source();
moved = 4;
let [one, two] = list();
let uninitialized;
let readBefore;
console.log(readBefore);
readBefore = 5;
for (let index = 0; index < 1; index++) {}
for (let key in object) {
  console.log(key);
}
for (let item of list()) {
  console.log(item);
}
for (let [left, right] of pairs()) {
  console.log(left, right);
}
let nested;
{
  nested = 6;
}
let inFunction;
function assign() {
  inFunction = 7;
}
let destructured, target;
({ destructured, target } = source());
let alpha, beta;
[alpha, beta] = list();
let gamma;
({ gamma, delta: object.property } = source());
let counter = 0;
counter++;
let compound = 1;
compound += 1;
var old = 1;
var reused = 1;
reused = 2;
switch (old) {
  case 1:
    var inCase = 2;
    break;
}
export {
  never,
  reassigned,
  later,
  both,
  other,
  partly,
  changed,
  first,
  second,
  kept,
  moved,
  one,
  two,
  uninitialized,
  readBefore,
  nested,
  inFunction,
  assign,
  destructured,
  target,
  alpha,
  beta,
  gamma,
  counter,
  compound,
  old,
  reused,
  inCase,
};
