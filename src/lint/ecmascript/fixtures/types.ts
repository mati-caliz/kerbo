// @ts-nocheck
/// @ts-check
//// @ts-ignore
/* @ts-ignore */
/*
 * @ts-expect-error
 */
/**
 * texto
 * @ts-expect-error con una descripción
 */
// @ts-expect-error ab
// @ts-expect-error abc
// @ts-ignorar
//@ts-ignore
// algo @ts-ignore
import type { Shape } from "./shapes";

let value: any = input!;
const self = this;
let other;
other = this;
const { alias } = this;
other.field = this;
const wrapped = (this);

interface Empty {}
interface Single extends Shape {}
interface Double extends Shape, Other {}
interface Filled { name: string }
interface Commented {
  // nada
}
type Literal = {};
type Mixed = Shape & {};
type Grouped = Shape & ({});
type Union = Shape | {};
function take(input: {}, list: Array<any>, rest: any[]): any {
  return input!.value!!.other as any;
}
const mapped: { [K in keyof Shape]: Shape[K] } = value;
class Holder {
  field!: string;
  method(this: any): void {
    this.field! = "x";
  }
}

namespace Space {}
declare module "external" {}
declare global {}
export { value, self, other, alias, wrapped, take, mapped, Holder };
export type { Empty, Single, Double, Filled, Commented, Literal, Mixed, Grouped, Union };

function precedence(a: boolean, value: number, map: Map<string, number>, key: string) {
  if (!a || value < map.get(key)!) {
    return !map.get(key)!.toFixed();
  }
  return typeof map.get(key)! + -map.size!;
}
export { precedence };
