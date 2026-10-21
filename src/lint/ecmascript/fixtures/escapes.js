"use strict";
"\a directive";
export const strings = [
  "\a\b\c\d\e",
  '\'\"',
  "\'\"",
  "\\\-",
  "\0",
  "line \
continued",
  `\a \` \$ \${ \{ $\{ \0`,
  `${strings} \- ${strings}\:`,
  tag`\a\-`,
];
export const patterns = [
  /\a\b\c\d\-\:\/\./,
  /[\a\b\-\^\]\/\.\[]/,
  /[\-a]/,
  /[a\-]/,
  /[a\-b]/,
  /[\^a]/,
  /[^\^a]/,
  /\k<name>(?<name>x)\k<name>/,
  /\k/,
  /\p{L}\-/u,
  /\-/u,
  /[\-]/u,
  /\x41A\cA\cZ\0\12/,
  /a{2,1}\-/,
  /(?:\-)(?=\:)(?<=\;)/,
  /\x\u\c/,
  /[z-a]\-/,
  /a**\-/,
];
const element = <a title="\d">\d</a>;
export function prologue() {
  "use\ strict";
  return element;
}
export async function statements(value) {
  value;
  value.property;
  value?.call();
  value();
  value && run();
  void value;
  delete value.property;
  !value;
  `template`;
  tag`template`;
  new Thing();
  value = 1;
  value++;
  await value;
  (value, run());
  import("./module");
  <div />;
  [value];
  "not a directive";
}
