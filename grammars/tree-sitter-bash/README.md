# tree-sitter-bash con las formas que le faltan

Copia de tree-sitter-bash 0.25.1 (la última publicada), porque la de arriba deja con error de sintaxis código
válido y kerbo saltea los scripts enteros en esos casos:

- `for x do` sin `in` ni terminador, `3<>archivo`, `${a:$b:$c}` y `${@:$#}`, `((3.14 * c))`, `a[0]` adentro de un
  `((...))`, `esac` después de `;&` en la misma línea y `${ cmd; }`.
- `grep 404$;`, con el `$` suelto antes de `;`, `|`, `&`, `)` o `/`.
- `V=3 kube::log::status`: una palabra con `::` no es una variable seguida de `:`.
- `var[i]` sólo es un subíndice si después viene una asignación o un `}`.
- `${a%%]*}`, `${a//$b\/$c\//}`, `${a//'x/y'/z}` y `${a:-x<1}`: los patrones de una expansión aceptan lo que el
  lexer común corta.
- `*"a"*"b"*)` en un `case`, `[[ $a = -* ]]` y `[[ $a = *".x" || $b = *".y" ]]`.
- `[ a \< b ]`, `[[ a \< b ]]` y `[ a '<' b ]` como comparaciones.

`src/parser.c` se regenera con `npx tree-sitter-cli@0.25.9 generate`; el corpus de tests de upstream pasa entero (100
de 100).
