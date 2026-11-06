function usesDeletes(a) {
  delete a;
  delete a.b;
  delete (a);
}
var octalNumber = 0123 + 09 + 0 + 0.5 + 0x10;
var decimalEscapes = "\8" + '\9' + "\\8" + "a\8b\9c" + "\0" + `\\8`;
with (octalNumber) {
  usesDeletes(decimalEscapes);
}
function duplicates(a, a, b) { return [a, b]; }
var expression = function (x, y, x) { return y; };
var arrow = function (p, q) { return [p, q]; };
usesDeletes(duplicates, expression, arrow);
