import type { OnlyType } from "./types";
import { type Inline, Value, Unused, Typed, Queried } from "./values";
import fs = require("fs");
import Alias = Space.Inner;

interface UnusedInterface {}
interface SelfReferencing {
  next: SelfReferencing;
}
interface Generic<T, U> {
  value: T;
}
type Alias2<K> = K;
type UnusedType = string;
type UsedType = number;
let typed: UsedType = 1;
console.log(typed);

enum Color {
  Red,
  "Green",
  Blue = Red,
}
enum UnusedEnum {
  A,
}
const enum Direction {
  Up,
}
console.log(Direction.Up);

namespace UnusedSpace {
  export const member = 1;
}
namespace UsedSpace {
  const hidden = 1;
  export const visible = 2;
}
console.log(UsedSpace.visible);
declare namespace Ambient {
  const ambientValue: number;
  function ambientFunction(argument: string): void;
}
declare module "external" {
  interface Augmented {
    field: string;
  }
}
declare global {
  interface Window {
    custom: string;
  }
}

class Service {
  constructor(private readonly client: Value, public name: string, unusedArgument: number) {}
  overload(value: string): void;
  overload(value: number): void;
  overload(value: unknown) {}
}
new Service(new Value(), "", 1);

function guard(input: unknown): input is string {
  return true;
}
guard(1);
function asserts(input: unknown): asserts input {}
asserts(1);
function withThis(this: Window, other: number) {}
withThis.call(window, 1);

type Callback = (error: Error, result: string) => void;
type Mapped<T> = { [Key in keyof T]: T[Key] };
type Inferred<T> = T extends Array<infer Item> ? Item : never;
type UnusedInfer<T> = T extends Array<infer Element> ? true : false;
let callback: Callback = () => {};
let mapped: Mapped<{}> = {};
let inferred: Inferred<[]>;
let unusedInfer: UnusedInfer<[]>;
let queried: typeof Queried;
let typedValue: Typed;
let only: OnlyType;
let inline: Inline;
console.log(callback, mapped, inferred, unusedInfer, queried, typedValue, only, inline);

abstract class Shape {
  abstract area(scale: number): number;
}
export { Shape };
export type { Generic, Alias2 };
const valueAndType = 1;
type valueAndType = string;
export type { valueAndType };
function generic<Unused2>() {}
generic();
fs;
let assertion = 0;
assertion! = 1;
let casted = 0;
(casted as number) = 1;
export function assertion1<T>(_: any): asserts _ is T {}
export function assertion2(value: unknown): asserts value {}
export function guard1(value: unknown): value is string {
  return true;
}
export function guard2(first: unknown, second: unknown): first is string {
  return true;
}
