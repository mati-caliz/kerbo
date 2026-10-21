import React from "react";
import { used, unused as renamed, other } from "./module";
import * as everything from "./everything";
import fallback from "./fallback";

const declared = 1;
let assigned = 1;
assigned = 2;
let incremented = 0;
incremented++;
let added = 0;
added += 1;
let selfAssigned = 0;
selfAssigned = selfAssigned + 1;
let readLater = 0;
readLater = 1;
console.log(readLater);
var hoisted;
let [first, , third = used] = [];
const { a, b: { c }, ...rest } = {};
const { d = other } = {};

function unusedFunction() {}
function recursive(count) {
  return count > 0 ? recursive(count - 1) : 0;
}
function usedFunction(firstArgument, secondArgument, thirdArgument) {
  return secondArgument;
}
function destructured({ x, y }, [z]) {
  return y;
}
function defaults(alpha = 1, beta) {
  return beta;
}
function restParameter(gamma, ...others) {
  return gamma;
}
usedFunction();
destructured();
defaults();
restParameter();

const arrow = (value) => {};
const named = function inner() {
  return inner;
};
const assignedArrow = () => assignedArrow();
class Unused {}
class Used {
  method(parameter) {}
  set value(next) {}
  static create() {
    return new Used();
  }
}
new Used();

try {
  run();
} catch (error) {}
try {
  run();
} catch {}

for (const key in object) {
  return;
}
for (const item of list) {
}
for (const entry of list) {
  console.log(entry);
}
for (let index = 0; index < 1; index++) {}

function closure() {
  let counter = 0;
  return () => counter++;
}
closure();

function stored() {
  let timer;
  timer = setTimeout(() => timer, 1);
}
stored();

function shadowing() {
  const value = 1;
  {
    const value = 2;
    console.log(value);
  }
}
shadowing();

const element = <div />;
export { element, React as default };
export const exported = 1;
export function exportedFunction(unusedParameter) {}

let chained = 0;
chained = chained || 1;
let logical = 0;
logical ||= 1;
let sequence = 0;
(sequence = 1), run();
label: for (;;) {
  break label;
}
var redeclared = 1;
var redeclared = 2;
const { [computedKey]: picked } = {};
fallback;
