import fs = require("fs");
import type path = require("path");
const loaded = require("./loaded");
const optional = require?.("./optional");
const wrapped = (require)("./wrapped");
const tagged = require`./tagged`;
declare var declared: number;
declare global {
  var augmented: number;
}
export { fs, loaded, optional, wrapped, tagged };
export type { path };
