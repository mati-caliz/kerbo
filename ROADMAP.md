# Hoja de ruta

kerbo quiere hacer lo que hoy hacen por separado Checkstyle, PMD, SpotBugs, ESLint, ruff, mypy,
golangci-lint y los formateadores, con un solo binario, una sola configuración y un solo reporte,
para Java, TypeScript/JavaScript, Python y Go.

## Principios

- **Paridad medida, no aproximada.** Cada regla se compara contra la herramienta que reemplaza
  sobre repos reales y tiene que marcar exactamente lo mismo. Mientras una regla o un lenguaje no
  llegue, se sigue usando la herramienta original: un resultado "parecido" no sirve para un gate.
- **Liviano de verdad.** Un ejecutable nativo, sin runtimes ni plugins, que analiza archivo por
  archivo en paralelo. La meta es menos de 200 MB en el monorepo más grande y segundos en vez de
  minutos.
- **La vara más alta de fábrica.** El preset por defecto es estricto. Se puede aflojar una regla
  por repo, pero siempre con un motivo escrito que aparece en el reporte: nada se afloja en
  silencio.
- **Pensado también para agentes de IA.** Salida compacta y priorizada, identificadores estables
  por problema, arreglos como parche y avisos cuando un cambio esquiva una regla en vez de
  arreglarla.
- **No se reimplementan compiladores ni runners de tests.** Los tipos de TypeScript vienen del
  compilador; los tests corren con el runner de cada lenguaje, pero sólo los que afecta el cambio.

## Qué hay hoy (0.1)

Las cinco reglas universales, escritas una vez sobre el árbol de tree-sitter y aplicadas en los
cuatro lenguajes: largo de archivo, largo de función, complejidad ciclomática, complejidad
cognitiva y cantidad de parámetros. Cada lenguaje cuenta como la herramienta que se usa hoy en
él:

| Lenguaje | Imita a |
|---|---|
| TypeScript / JavaScript | ESLint (`complexity`, `max-lines`, `max-lines-per-function`, `max-params`) y sonarjs (`cognitive-complexity`) |
| Python | ruff (`C901`, `PLR0913`) |
| Java | Checkstyle (`CyclomaticComplexity`, `MethodLength`, `ParameterNumber`, `FileLength`) y PMD (`CognitiveComplexity`) |
| Go | golangci-lint (`cyclop`, `gocognit`, `funlen`, `revive`) |

Esas herramientas no cuentan igual, y las diferencias no están documentadas: ESLint suma `??` a la
ciclomática y sonarjs no lo suma a la cognitiva, ruff no cuenta `self` ni `cls`, Checkstyle mide
un método desde la llave y saltea los `@Override`, funlen cuenta las líneas de adentro de la
función con blancos y comentarios. Cada una está fijada en un test.

Medido sobre diez repos de producción (unos 12.000 archivos): cero diferencias con las
herramientas originales, entre 0,06 y 2,3 s por repo y menos de 12 MB de memoria.

## Qué sigue

1. **Paridad en las dos direcciones.** Hoy está probado que kerbo no marca de más; falta bajar
   los umbrales en las dos herramientas y comparar función por función, para probar que tampoco
   marca de menos.
2. **Núcleo.** Archivo de configuración (`kerbo.toml`) con presets y excepciones con motivo,
   caché por contenido, salida SARIF, y más reglas universales: anidamiento, números mágicos,
   nombres cortos, bloques duplicados.
3. **Capa para agentes.** `--format agent`, identificadores estables, `kerbo explain <regla>`,
   línea de base, modo diff contra `main`, servidor MCP y las reglas antitrampa (supresiones
   nuevas, umbrales bajados, tests borrados o salteados).
4. **Java sintáctico.** Las reglas de forma de Checkstyle y PMD, con paridad medida.
5. **Flujo de datos sin compilar.** Lo de SpotBugs sobre el código fuente: nulos, recursos sin
   cerrar, excepciones tragadas, contaminación de entradas hacia SQL, comandos y rutas.
6. **Go, Python y TypeScript** completos, cada uno contra su herramienta.
7. **Tests inteligentes, dependencias y secretos.** Correr sólo los tests afectados, reusar
   resultados, vulnerabilidades con OSV.
8. **Formateo propio**, lenguaje por lenguaje y sólo cuando la salida sea idéntica a la de la
   herramienta actual.
9. **Editor**: servidor LSP.
