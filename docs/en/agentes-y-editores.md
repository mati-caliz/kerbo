# AI agents and editors

## For AI agents

- `kerbo check --format agent`: short, capped output, cheats first and then what exceeds the
  limit the most. It groups by file (the path appears once), each problem is a row whose columns
  are declared once (TOON style) with a short, stable `id`, and how to fix each rule appears once
  at the end. It spends 2.1 to 3.7 times fewer tokens than the previous JSON.
  `--format agent-json` gives the same thing as JSON, for code that parses it; the hook and the
  MCP server return `--format agent`.
  When the fix is mechanical (a redundant import, an extra modifier, a missing `final`), the
  `patch` section carries, by `id`, the unified diff hunk that solves it (in `agent-json`, the
  problem's `patch` field), and the last line, `autofix[N]: kerbo fix`, says how many problems of
  the whole report `kerbo fix` solves on its own.
- `kerbo explain <rule>`: why it exists, how to fix it and an example, short enough for the
  context window.
- **Claude Code hook**: reviews every file the agent edits, in milliseconds, and if it finds
  something it hands it back so the agent fixes it in the same turn. In `.claude/settings.json`:

  ```json
  {
    "hooks": {
      "PostToolUse": [
        { "matcher": "Edit|MultiEdit|Write", "hooks": [{ "type": "command", "command": "kerbo hook --lang en" }] }
      ]
    }
  }
  ```

  In a git repo it only looks at what changed against `HEAD`, anti-cheat included, and it honors
  the baseline: the agent does not start fixing code it did not touch. If kerbo fails, the hook
  does not block.
- **Daemon** (`kerbo daemon start`, `status` and `stop`): one process per user and machine, with
  the cache in memory, that `kerbo check` and the hook hand their work to, so several agent
  sessions on the same machine do not start from scratch on every edit. The hook starts it the
  first time. Requests from the same repo run in parallel; those from another repo wait their
  turn. It only listens on `127.0.0.1`, with a token kept in the cache folder, and stops itself
  after 30 idle minutes. If it is not running, or it belongs to another kerbo version, every
  command runs on its own as always, with the same output. `KERBO_DAEMON=off` leaves it out.
- **MCP server** (`kerbo mcp`, over stdio) with three tools: `kerbo_check` (folders, `diff` and
  languages; returns the output for agents), `kerbo_fix` (the same as `kerbo fix`, with folders,
  rules, languages and `dry_run`; returns the patch and the summary) and `kerbo_explain`. In Claude Code:
  `claude mcp add kerbo -- kerbo mcp --lang en`; in Cursor and other clients, a stdio server with
  the command `kerbo` and the arguments `mcp --lang en`.

## In the editor

`kerbo lsp` is an LSP server: the editor passes it the file while you type and kerbo flags the
problems instantly, without waiting for you to save, with the repo's `kerbo.toml` and baseline.
The ones with a mechanical fix show up as a quick fix: delete the import, drop the modifier or add
the `final`. If the editor sends the problems it has on screen (`context.diagnostics`), only those
are offered; otherwise, the ones on the requested lines. The `source.fixAll.kerbo` action fixes
everything mechanical in the file that does not overlap at once (in VS Code,
`"editor.codeActionsOnSave": { "source.fixAll": "explicit" }` runs it on save).

- **IntelliJ IDEA, PyCharm, GoLand, WebStorm** (Community or Ultimate): install the
  [LSP4IJ](https://plugins.jetbrains.com/plugin/23257-lsp4ij) plugin, and in *Settings →
  Languages & Frameworks → Language Servers* add a new one with the command `kerbo lsp` and, in
  *Mappings → File name patterns*, `*.java`, `*.ts`, `*.tsx`, `*.js`, `*.py` and `*.go`.
- **Neovim**:

  ```lua
  vim.api.nvim_create_autocmd("FileType", {
    pattern = { "java", "typescript", "typescriptreact", "javascript", "python", "go" },
    callback = function() vim.lsp.start({ name = "kerbo", cmd = { "kerbo", "lsp" } }) end,
  })
  ```

- **Any editor with a generic LSP client** (Zed, Helix, Sublime with LSP): the command
  `kerbo lsp`, over stdio.
