var simple = 1;
var first = 1, second = 2;
var uninitialized;
var redeclared = 1;
var redeclared = 2;
var usedBefore = before();
function before() {
  return usedBefore;
}
var self = self + 1;
var fn = function () {
  return fn;
};
for (var index = 0; index < 3; index++) {
  setTimeout(() => index);
}
for (var counter = 0; counter < 3; counter++) {
  console.log(counter);
}
for (var key in object) {
  console.log(key);
}
for (var item of list) {
  console.log(() => item);
}
while (simple) {
  var inLoop;
  var initialized = 1;
}
if (simple) {
  var inBlock = 1;
}
console.log(inBlock);
{
  var scoped = 1;
  console.log(scoped);
}
try {
  run();
} catch (error) {
  var error = 1;
}
switch (simple) {
  case 1:
    var inCase = 1;
}
label: var labeled = 1;
var let_ = 1;
function hoisted() {
  call();
  var late = 1;
  function call() {
    return late;
  }
}
export var exported = 1;
const require = (name) => name;
const local = require("./local");
function outer() {
  const loaded = require("./loaded");
  return loaded;
}
function shadowed(require) {
  return require("./shadowed");
}
function args() {
  return arguments.length + arguments[0] + Array.from(arguments).length;
}
const arrow = () => arguments;
function nested() {
  return () => arguments;
}
function declared(arguments) {
  return arguments;
}
export {
  simple,
  first,
  second,
  uninitialized,
  redeclared,
  self,
  fn,
  key,
  item,
  inLoop,
  initialized,
  error,
  inCase,
  labeled,
  let_,
  hoisted,
  local,
  outer,
  shadowed,
  args,
  arrow,
  nested,
  declared,
};
