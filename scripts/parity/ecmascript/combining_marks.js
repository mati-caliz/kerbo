// Genera src/lint/ecmascript/combining_marks.rs: los rangos de `\p{M}` que `isCombiningCharacter`
// de ESLint pregunta al motor de JavaScript. Uso: node combining_marks.js > ../../../src/lint/ecmascript/combining_marks.rs
const LAST_CODE_POINT = 0x10ffff;
const isMark = /^\p{M}$/u;
const ranges = [];
let start = null;
for (let codePoint = 0; codePoint <= LAST_CODE_POINT + 1; codePoint += 1) {
  const marked = codePoint <= LAST_CODE_POINT && isMark.test(String.fromCodePoint(codePoint));
  if (marked && start === null) {
    start = codePoint;
  } else if (!marked && start !== null) {
    ranges.push([start, codePoint - 1]);
    start = null;
  }
}
const hex = (value) => `0x${value.toString(16)}`;
const lines = ranges.map(([first, last]) => `    (${hex(first)}, ${hex(last)}),`);
console.log(`// Generado por scripts/parity/ecmascript/combining_marks.js: \`\\p{M}\` de Unicode ${process.versions.unicode}.
const COMBINING_MARK_RANGES: [(u32, u32); ${ranges.length}] = [
${lines.join("\n")}
];

pub fn is_combining_mark(code_point: u32) -> bool {
    COMBINING_MARK_RANGES
        .binary_search_by(|(first, last)| {
            if code_point < *first {
                std::cmp::Ordering::Greater
            } else if code_point > *last {
                std::cmp::Ordering::Less
            } else {
                std::cmp::Ordering::Equal
            }
        })
        .is_ok()
}`);
