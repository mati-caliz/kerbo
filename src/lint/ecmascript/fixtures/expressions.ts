namespace Space {
  "use strict";
  export const inner = 1;
}
export function typed(value: unknown, call: () => void) {
  value as string;
  value!;
  <string>value;
  value satisfies unknown;
  call as () => void;
  (call as () => void)();
  call!();
  typed<string>;
  call() satisfies Space.Inner.Type;
}
export type Pattern = `a${string}b`;
export const literal: "\d" = "\d";
