/* global first, second: writable */
/* globals third fourth:readonly -- declarados por el entorno */
/* global fifth: off */
// global sixth

console.log(first, second, third, fourth, fifth, sixth);

if (typeof missing === "undefined") {
  implicit = 1;
}

function uses() {
  return implicit + window.innerWidth + process.env.HOME + require("x") + describe;
}

var declaredLater;
declaredLater = notDeclared;
uses(typeof (alsoMissing), declaredLater);
