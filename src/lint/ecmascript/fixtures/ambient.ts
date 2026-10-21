declare function instantiate<M>(id: string, source: M): M;
declare const flag: boolean;
declare class Holder {}
declare namespace Outer {
  namespace Inner {
    const deep: number;
  }
  const shallow: number;
  export const exported: number;
}
declare module "virtual" {
  const hidden: number;
  export { hidden };
}
async function disposable() {
  using resource = open();
  await using pending = await open();
  using kept = open();
  kept.close();
}
disposable();
