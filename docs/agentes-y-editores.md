# Agentes de IA y editores

## Para agentes de IA

- `kerbo check --format agent`: salida corta con tope, las trampas primero y después lo que más
  se pasa del límite. Agrupa por archivo (la ruta va una vez), cada problema es una fila con
  columnas declaradas una sola vez (al estilo de TOON) y un `id` corto y estable, y cómo se
  arregla cada regla va una sola vez al final. Gasta entre 2,1 y 3,7 veces menos tokens que el
  JSON de antes. `--format agent-json` da lo mismo en JSON, para quien lo lee con código; el hook y
  el servidor MCP devuelven `--format agent`.
  Si el arreglo es mecánico (un import que sobra, un modificador de más, un `final` que falta),
  la sección `patch` trae, por `id`, el bloque de diff unificado que lo resuelve (en
  `agent-json`, el campo `patch` del problema), y la última línea, `autofix[N]: kerbo fix`, dice
  cuántos problemas del reporte entero arregla `kerbo fix` solo.
- `kerbo explain <regla>`: por qué existe, cómo se arregla y un ejemplo, corto para el contexto.
- **Hook de Claude Code**: revisa cada archivo que el agente edita, en milisegundos, y si encuentra
  algo se lo devuelve para que lo corrija en la misma vuelta. En `.claude/settings.json`:

  ```json
  {
    "hooks": {
      "PostToolUse": [
        { "matcher": "Edit|MultiEdit|Write", "hooks": [{ "type": "command", "command": "kerbo hook" }] }
      ]
    }
  }
  ```

  En un repo git sólo mira lo que cambió respecto de `HEAD`, antitrampa incluida, y respeta la línea
  de base: el agente no se pone a arreglar código que no tocó. Si kerbo falla, el hook no bloquea.
- **Demonio** (`kerbo daemon start`, `status` y `stop`): un proceso por usuario y máquina, con la
  caché en memoria, al que `kerbo check` y el hook le pasan el trabajo. Así varias sesiones de
  agentes en la misma máquina no arrancan todo de cero en cada edición. El hook lo arranca solo la
  primera vez. Atiende en paralelo los pedidos del mismo repo; los de otro repo esperan su turno.
  Escucha sólo en `127.0.0.1`, con un token que queda en la carpeta de caché, y se apaga solo
  después de 30 minutos sin uso. Si no está corriendo, o es de otra versión de kerbo, cada comando
  corre solo como siempre, y la salida es la misma. `KERBO_DAEMON=off` lo deja afuera.
- **Servidor MCP** (`kerbo mcp`, por stdio) con tres herramientas: `kerbo_check` (carpetas, `diff`
  y lenguajes; devuelve la salida para agentes), `kerbo_fix` (lo mismo que `kerbo fix`, con
  carpetas, reglas, lenguajes y `dry_run`; devuelve el parche y el resumen) y `kerbo_explain`. En Claude Code:
  `claude mcp add kerbo -- kerbo mcp`; en Cursor y otros clientes, un servidor stdio con el
  comando `kerbo` y el argumento `mcp`.

## En el editor

`kerbo lsp` es un servidor LSP: el editor le pasa el archivo mientras escribís y kerbo marca los
problemas al instante, sin esperar a que guardes, con el `kerbo.toml` y la línea de base del repo.
Los que tienen arreglo mecánico aparecen como acción rápida (*quick fix*): borrar el import,
sacar el modificador o agregar el `final`. Si el editor manda los problemas que tiene a la vista
(`context.diagnostics`), sólo se ofrecen los de esos; si no, los de las líneas pedidas. La acción
`source.fixAll.kerbo` arregla de una vez todo lo mecánico del archivo que no se pisa (en VS Code,
`"editor.codeActionsOnSave": { "source.fixAll": "explicit" }` lo corre al guardar).

- **IntelliJ IDEA, PyCharm, GoLand, WebStorm** (Community o Ultimate): instalá el plugin
  [LSP4IJ](https://plugins.jetbrains.com/plugin/23257-lsp4ij), y en *Settings → Languages &
  Frameworks → Language Servers* agregá uno nuevo con el comando `kerbo lsp` y, en *Mappings →
  File name patterns*, `*.java`, `*.ts`, `*.tsx`, `*.js`, `*.py` y `*.go`.
- **Neovim**:

  ```lua
  vim.api.nvim_create_autocmd("FileType", {
    pattern = { "java", "typescript", "typescriptreact", "javascript", "python", "go" },
    callback = function() vim.lsp.start({ name = "kerbo", cmd = { "kerbo", "lsp" } }) end,
  })
  ```

- **Cualquier editor con LSP genérico** (Zed, Helix, Sublime con LSP): comando `kerbo lsp`, por
  stdio.
