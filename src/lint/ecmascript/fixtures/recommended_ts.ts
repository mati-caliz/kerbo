export enum Values {
  A = 1,
  B = 1,
  C = "x",
  D = "x",
  E = `x`,
  F = -1,
  G = -1,
  H = +1,
  I,
  J = 1 + 1,
  K = 1 + 1,
}
export enum Others {
  A = "1",
  B = 1,
  C = 0x1,
}
declare const a: { b: string } | null;
export const one = a!!;
export const two = a!?.b;
export const three = (a!)?.b;
declare const f: (() => void) | null;
f!?.();
(f!)?.();
export const four = (a!)!;
export const five = a!.b;
export interface Misused {
  new (): Misused;
  constructor(): void;
  build(): Misused;
}
export interface Fine {
  new (): string;
}
export type Literal = { constructor(): void; new (): Literal };
export declare class Built {
  new(): Built;
  new(): string;
}
export class Bodied {
  new(): Bodied {
    return this;
  }
}
export function constraints<T extends any, U extends unknown, V extends string, W>(): [T, U, V, W] {
  throw new Error();
}
export class Generic<T extends any> {
  value?: T;
}
export interface Merged {}
export class Merged {}
export class Alone {}
export interface Lonely {}
export interface Lonely {}
function scoped(): void {
  interface Inner {}
  class Inner {}
  class Other {}
  new Inner();
  new Other();
}
scoped();
export const asLiteral = "a" as "a";
export const asOther = "a" as "b";
export const asNumber = 1 as 1;
export const asTrue = true as true;
export const assertion = <"a">"a";
export let declared: "a" = "a";
export let other: "a" = "b";
export let widened: string = "a";
export let quoted: "a" = 'a';
export class Fields {
  field: 2 = 2;
  other: 2 = 3;
  plain = 2;
}
module Old {}
declare module Ambient {}
declare module "quoted" {}
namespace New {}
declare global {}
module Dotted.Name {}
export { Old, New };
