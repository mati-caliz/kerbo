// Estándar de calidad: lo copia quality/sync.sh y no se edita en el repo.
// Lee rutas de archivos TypeScript por la entrada estándar e imprime sus identificadores, uno por
// línea y separado de su archivo con un tabulador. Usa el parser de TypeScript, así que no entran strings, comentarios ni texto
// de JSX.
//   node identifiers.mjs <paquete-con-typescript> < archivos
import { readFileSync } from "node:fs";
import { createRequire } from "node:module";
import path from "node:path";

const [packageDir] = process.argv.slice(2);
const ts = createRequire(path.join(packageDir, "package.json"))("typescript");
const files = readFileSync(0, "utf8").split("\n").filter(Boolean);
const lines = [];

function identifiersOf(file) {
  const names = new Set();
  const collect = (node) => {
    if (ts.isIdentifier(node) || ts.isPrivateIdentifier(node)) {
      names.add(node.text);
    }
    ts.forEachChild(node, collect);
  };
  const kind = file.endsWith("x") ? ts.ScriptKind.TSX : ts.ScriptKind.TS;
  collect(ts.createSourceFile(file, readFileSync(file, "utf8"), ts.ScriptTarget.Latest, false, kind));
  return names;
}

for (const file of files) {
  for (const name of identifiersOf(file)) {
    lines.push(`${file}\t${name}`);
  }
}
process.stdout.write(lines.join("\n") + "\n");
