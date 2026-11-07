# Reglas

| Regla | TypeScript / JavaScript | Python | Java | Go | Rust |
|---|---|---|---|---|---|
| `file-length` | 400 líneas de código | — | 500 líneas | 400 líneas | — |
| `function-length` | 120 líneas de código | — | 80 líneas desde la llave | 80 líneas internas | 80 líneas con código del cuerpo, contadas sobre el texto |
| `cyclomatic-complexity` | 10 | 10 | 8 | 10 | — |
| `cognitive-complexity` | 15 | — | 15 | 15 | 15, la de clippy y no la de Sonar |
| `parameter-count` | 4 | 6 (sin `self` ni `cls`) | 5 (sin `@Override`) | 4 | 4 (con el receptor, sin implementaciones de un trait) |
| `nesting-depth` | 4 bloques | — | — | 4 bloques de control | 4 bloques, `impl`, `trait` o módulos |
| `short-name` | menos de 2 caracteres salvo `i`, `j` y `_`, sin contar propiedades | `l`, `O` e `I` | locales de menos de 3 caracteres salvo `i` y `j` | menos de 3 caracteres si el último uso está a más de 5 líneas | — |
| `magic-number` | fuera de constantes de módulo, enums, tipos y defaults; -1, 0, 1 y 2 valen | sólo en comparaciones; 0 y 1 valen | fuera de campos, constantes y `hashCode`; -1, 0, 1 y 2 valen | en argumentos, asignaciones, casos, condiciones, operaciones y retornos; 0 y 1 valen | — |
| `duplicate-code` | funciones iguales de 3 líneas o más y ramas repetidas de un `if` o un `switch`, dentro del archivo | bloques de 100 tokens repetidos entre archivos o dentro de uno | lo mismo, sin contar anotaciones, `import` ni `;` | bloques de 150 nodos del árbol repetidos dentro de un paquete | — |
| `commented-code` | — | comentarios en su propia línea que compilan como Python | — | grupos de comentarios dentro de una función que compilan como una sentencia de Go | — |
| `unused-import` | — | — | imports que nadie nombra (los que sólo aparecen en el Javadoc cuentan como sin usar) y los de `java.lang`; se arregla solo | — | — |
| `redundant-import` | — | — | imports repetidos, de `java.lang` o del mismo paquete; se arregla solo | — | — |
| `star-import` | — | — | `import a.b.*`, también los `static` | — | — |
| `redundant-modifier` | — | — | lo que el lenguaje ya asume: `public`, `static`, `abstract` o `final` en una interfaz, `final` en un método privado o de una clase final, `static` en un enum o un record anidado, `public` en el constructor de una clase que no es pública; se arregla solo | — | — |
| `final-parameter` | — | — | parámetros de métodos y constructores con cuerpo que no son `final`; se arregla solo | — | — |
| `final-class` | — | — | clases con todos los constructores privados (o privadas y sin constructor) que no son `final` ni se heredan en el archivo; se arregla solo | — | — |

Cada lenguaje cuenta como su herramienta de referencia: ESLint y sonarjs, ruff, Checkstyle y PMD,
cyclop, gocognit, funlen, revive, mnd y varnamelen; los bloques duplicados, CPD de PMD, dupl y sonarjs (`no-identical-functions` y
`no-duplicated-branches`); el código comentado, ERA001 de ruff y `commentedOutCode` de gocritic; Rust, clippy. Esas diferencias de criterio no están documentadas en ningún
lado; cada una salió de comparar contra repos reales y quedó fijada en un test de
`src/metrics/tests.rs` (el anidamiento, los números mágicos y los nombres cortos, en los `tests.rs` de su módulo). En
los tests no se buscan números mágicos ni bloques duplicados, en los de TypeScript no se mide el largo de función y en
los de Python no se cuentan los parámetros. El preset `minimal` no busca números mágicos ni duplicados, y `recommended` marca
los bloques desde 150 tokens (225 nodos en Go y funciones de 5 líneas en TypeScript).

Esos son los umbrales del preset `strict`; los otros dos presets están en [Configuración](configuracion.md).

## Rust

En Rust, kerbo mide sólo lo que mide clippy (`too_many_lines`, `too_many_arguments`,
`cognitive_complexity` y `excessive_nesting`), y la complejidad cognitiva es la de clippy, no la de
Sonar: arranca en 1 y suma cada `if`, cada `match` de más de un brazo y cada guarda sobre el código
ya expandido, así que también suman los `for`, los `while`, cada `?` y cada `.await`, los `assert!`
y los `matches!`. Después descuenta los `return`: todos si la función devuelve `Result`, la mitad
si no. Los closures se miden aparte, también los que van adentro de una macro. Las `macro_rules!`
del propio crate se expanden como rustc, leyendo las definiciones de todos sus archivos, y también
las exportadas por las dependencias (`dep::macro!`, `use dep::macro` o `#[macro_use] extern
crate`), leídas de la caché de cargo con la versión del `Cargo.lock` o, sin lockfile, la más nueva
compatible con lo pedido. Las de la biblioteca estándar se imitan; de las macros procedurales se
cuentan sólo los argumentos.

Como clippy mide lo que compila, un ítem, una sentencia o un brazo de `match` con un `#[cfg]` falso
no cuenta para la complejidad ni para el anidamiento, y un archivo con `#![cfg(..)]` falso no se
mide. Los `#[cfg]` se evalúan con la máquina donde corre kerbo, todas las features del
`Cargo.toml` y `test` en los dos modos (la biblioteca y sus tests); lo que kerbo no conoce, como
un `cfg` que emite el `build.rs`, no decide y el código se mide. La sección `[rust]` de
`kerbo.toml` cambia la plataforma, las features y los `cfg` encendidos.
