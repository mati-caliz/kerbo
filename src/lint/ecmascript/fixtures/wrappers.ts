/// <reference types="node" />
/// <reference path="./globals.d.ts" />
/// <reference lib="es2020" />
/// <reference types="jest" />
/* <reference path="./ignored.d.ts" /> */
import { readFile } from "node";
import fs = require("node");

let text: String = "a";
let count: Number = 1;
let flag: Boolean = true;
let symbol: Symbol;
let big: BigInt;
let object: Object = {};
let list: Array<String> = [];
let nested: Promise<Number[]>;
let callback: Function;
let qualified: globalThis.String;
let queried: typeof String;

interface Callable extends Function {}
class Wrapper implements Object, Function {}
class Derived extends Object {}

namespace Shadowed {
  interface String {
    length: number;
  }
  export let shadowed: String;
}

function generic<Function>(value: Function): Function {
  return value;
}

type Boolean = { value: true };
let alias: Boolean;

function withSignature(callback: (value: Object) => Number): void {
  callback({});
}

const Number = 3;
function inner() {
  let local: Number;
  return local;
}

export { readFile, fs, text, count, flag, symbol, big, object, list, nested, callback, qualified, queried };
