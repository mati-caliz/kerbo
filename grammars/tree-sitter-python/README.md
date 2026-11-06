# tree-sitter-python con la sintaxis de Python 3.14 y 3.15

Copia de tree-sitter-python 0.25.0 (la última publicada), porque la de arriba deja con error de sintaxis
código válido y kerbo saltea las reglas de alcances en esos archivos:

- `lazy import x` y `lazy from x import y` (PEP 810). `lazy` sigue siendo un nombre válido en cualquier
  otro lado, como `match` y `type`, y queda como un token sin nombre adentro de la sentencia, así las
  reglas que leen los imports no cambian.
- Defaults de parámetros de tipo (PEP 696): `def f[T: int = str]()`, en un nodo aparte
  `type_parameter_default` para no confundirlos con otro parámetro.
- `a[*b, *(c())]`: un `*` con cualquier expresión en un subíndice.
- `a[:, 1]` en una anotación: un slice que empieza con `:` adentro de un `generic_type`.
- `case +0:` y `case +1 + 2j:`, que ruff y CPython aceptan.
- `src/scanner.c` sólo emite `DEDENT` donde el parser lo espera. Antes también lo emitía en cualquier línea
  menos indentada fuera de un paréntesis que se pudiera cerrar enseguida, y rompía `(bar.` con la línea
  siguiente menos indentada y un comentario sin sangría después de un decorador.

`src/parser.c` se regenera con `npx tree-sitter-cli@0.25.9 generate`, la misma versión que usó upstream; el
corpus de tests de upstream pasa entero (117 de 117).
