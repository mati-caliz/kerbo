# tree-sitter-go con la sintaxis de Go 1.26 y 1.27

Copia de tree-sitter-go 0.25.0 (la última publicada) con estos cambios en `grammar.js`, porque la de
arriba deja con error de sintaxis código válido de las versiones nuevas de Go y kerbo saltea las
reglas de Go en esos archivos:

- `new(expresión)` de Go 1.26 (`new(uint64(1))`): una alternativa más de `call_expression` con
  precedencia dinámica menor que la de `new(Tipo)`, así lo que ya parseaba como tipo no cambia.
- Métodos con parámetros de tipo de Go 1.27 (`func (m T) f[U any]()`): `method_declaration` suma
  `type_parameters`, como `function_declaration`.
- Lo que `go/parser` acepta y upstream no, que hacía que `kerbo fmt` dejara el archivo como estaba:
  - `chan<- chan T` es un canal de envío de `chan T` (el `<-` va con el `chan` de más a la
    izquierda): `chan T` sin dirección ya no toma un `<-chan` como valor sin paréntesis
    (`_type_after_chan`, `receive_channel_type`).
  - `if x {...}` seguido de un bloque suelto: el literal compuesto tiene precedencia dinámica
    negativa, así gana la lectura en la que la llave abre el cuerpo, que es la de Go.
  - Una función propia llamada `make` (`make(n - 1)`), `(make)(tipo, n)` y `(new)(tipo)`
    (`_parenthesized_builtin`), y `make(tipo, x...)`.
  - `const x T` sin valor, `[...]T` como tipo fuera de un literal y etiquetas seguidas antes de un
    `}` (`L: L1:`).

`src/parser.c` se regenera con `scripts/parity/go/grammar.sh generate` (tree-sitter-cli 0.25.8, la
misma versión que usó upstream); `grammar.sh test` corre el corpus de tests de upstream, que pasa
entero (67 de 67).
