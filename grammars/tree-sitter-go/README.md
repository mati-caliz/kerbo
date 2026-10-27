# tree-sitter-go con la sintaxis de Go 1.26 y 1.27

Copia de tree-sitter-go 0.25.0 (la última publicada) con dos cambios en `grammar.js`, porque la de
arriba deja con error de sintaxis código válido de las versiones nuevas de Go y kerbo saltea las
reglas de Go en esos archivos:

- `new(expresión)` de Go 1.26 (`new(uint64(1))`): una alternativa más de `call_expression` con
  precedencia dinámica menor que la de `new(Tipo)`, así lo que ya parseaba como tipo no cambia.
- Métodos con parámetros de tipo de Go 1.27 (`func (m T) f[U any]()`): `method_declaration` suma
  `type_parameters`, como `function_declaration`.

`src/parser.c` se regenera con `npx tree-sitter-cli@0.25.8 generate`, la misma versión que usó
upstream; el corpus de tests de upstream pasa entero (67 de 67).
