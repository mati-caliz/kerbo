# kerbo

[Español](README.md) · English

Code quality analyzer for **Java, TypeScript/JavaScript, Python, Go and Rust** in a single
binary. It applies the same rules to every language, in seconds and with a few megabytes of
memory, and counts exactly like the tool you already use for each one (ESLint, ruff, Checkstyle
and PMD, golangci-lint, clippy).

```console
$ kerbo check --lang en
src/billing/InvoiceService.java:88:5: cyclomatic-complexity The function `settle` has cyclomatic complexity 11 (maximum 8)
web/src/orders/use-cart.ts:42:1: cognitive-complexity The function `applyPromotions` has cognitive complexity 19 (maximum 15)
kerbo: 5810 files, 2 problems, 2250 ms
```

## Installation

Linux (x86_64, a static binary that runs on any distro):

```sh
curl -fsSL https://kerbo.matiascaliz.com.ar/install.sh | sh
```

The installer downloads the latest version from the
[releases](https://github.com/mati-caliz/kerbo/releases), verifies the sha256 and leaves `kerbo`
in `~/.local/bin` (another folder with `KERBO_INSTALL_DIR`).

macOS, Windows and other architectures, with Rust installed:

```sh
cargo install --git https://github.com/mati-caliz/kerbo --locked
```

## Usage

```sh
kerbo check                       # the current directory
kerbo check api web               # several folders or files
kerbo check --language java api   # a single stack: typescript, python, java or go
kerbo check --diff main           # only what the lines changed since main touch
kerbo check --format sarif        # also json, agent; GitHub and editors read sarif
kerbo baseline                    # accept what is already there: from now on only new problems
kerbo explain cognitive-complexity  # why a rule exists and how to fix it
```

It honors `.gitignore` and skips hidden files. It exits with `0` when there are no problems, `1`
when there are and `2` when something could not be read or the configuration is invalid.

Messages come out in Spanish or English. `--lang en` (or `es`) wins, then `lang` in
`kerbo.toml`, then the system locale (`LC_ALL`, `LC_MESSAGES`, `LANG`); without any of them,
Spanish. The language only changes the text meant for people: rule identifiers, stable `id`s and
the shape of the JSON, SARIF and `--format agent` output stay the same.

### Adopting it in an existing repo

`kerbo baseline` saves the current problems in `kerbo-baseline.json` (commit it). From then on,
`kerbo check` reports only new problems and the ones that **got worse**: if a function in the
baseline goes from complexity 14 to 16, it shows up again. Each problem is identified by rule,
file and function, not by line number, so moving code around does not make it "new".
`--no-baseline` shows everything.

### Anti-cheat

With `--diff`, kerbo also looks at **how** a problem was solved. It flags what a change adds to
dodge a rule instead of fixing it, only on the new lines:

| Rule | What it detects |
|---|---|
| `suppression-added` | `eslint-disable`, `@ts-ignore`, `noqa`, `type: ignore`, `nolint`, `#nosec`, `@SuppressWarnings`, `NOPMD`, `CHECKSTYLE:OFF`, `istanbul ignore`, `NOSONAR`, `#[allow]`, `#[expect]`…, and a threshold loosened in the configuration: a maximum that goes up, a minimum or a coverage that goes down, a rule turned `off` or `allow`, `strict` turned `false` or a looser preset |
| `test-disabled` | `it.skip`, `it.only`, `xit`, `describe.only`, `@Disabled`, `@Ignore`, `@pytest.mark.skip`, `t.Skip()`, `#[ignore]`…, and a test left empty |
| `test-removed` | deleted tests, compared against the reference; if the tests only moved to another file, it stays quiet |
| `type-escape` | `any` and `as unknown as` in TypeScript |

Everything is detected on the syntax tree: an `eslint-disable` inside a string does not count. If
an exception is legitimate, it goes in `kerbo.toml` with its `reason`, in plain sight.

### For AI agents

- `kerbo check --format agent`: compact, capped JSON, cheats first and then what exceeds the
  limit the most, each problem with a stable identifier and a fix hint.
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
- **MCP server** (`kerbo mcp`, over stdio) with two tools, `kerbo_check` (folders, `diff` and
  languages; returns the output for agents) and `kerbo_explain`. In Claude Code:
  `claude mcp add kerbo -- kerbo mcp --lang en`; in Cursor and other clients, a stdio server with
  the command `kerbo` and the arguments `mcp --lang en`.

### In the editor

`kerbo lsp` is an LSP server: the editor passes it the file while you type and kerbo flags the
problems instantly, without waiting for you to save, with the repo's `kerbo.toml` and baseline.

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

### `kerbo.toml`

Optional, at the root of the repo (kerbo looks for it upwards from the current folder). Without
it the `strict` preset applies. **Every adjustment carries its reason**, and the summary counts
the deviations from the preset: nothing gets loosened silently.

```toml
preset = "recommended"            # strict (the default), recommended or minimal
lang = "en"                       # optional: es or en; when missing, the system's

[[limits]]                        # a threshold for every language, or for one
rule = "cyclomatic-complexity"
max = 12
language = "java"                 # optional: typescript, python, java or go
reason = "the team agreed on 12 for the services"

[[exceptions]]                    # per folder or file
paths = ["src/legacy/**"]
rule = "cognitive-complexity"     # without `max`, the rule does not run there; without `rule`, none does
reason = "it gets rewritten in the first quarter"

[[ignore]]
paths = ["generated/**"]
reason = "generated by the protobuf compiler"
```

| Preset | File | Function | Cyclomatic | Cognitive | Parameters |
|---|---|---|---|---|---|
| `strict` | the table below, per language | | | | |
| `recommended` | 750 | 150 | 15 | 20 | 6 |
| `minimal` | 1500 | 300 | 25 | 30 | 8 |

### GitHub Actions

```yaml
permissions:
  contents: read
  security-events: write          # to annotate the problems in the pull request

jobs:
  kerbo:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
        with:
          fetch-depth: 0            # for --diff
      - uses: mati-caliz/kerbo@v0.2.0
        with:
          args: --lang en --diff origin/${{ github.base_ref }}
```

The action installs the Linux binary, uploads the SARIF report (the problems show up in the
pull request's changes tab) and fails the job when there are problems.

## Rules

| Rule | TypeScript / JavaScript | Python | Java | Go | Rust |
|---|---|---|---|---|---|
| `file-length` | 400 lines of code | — | 500 lines | 400 lines | — |
| `function-length` | 120 lines of code | — | 80 lines from the brace | 80 inner lines | 80 lines with body code, counted on the text |
| `cyclomatic-complexity` | 10 | 10 | 8 | 10 | — |
| `cognitive-complexity` | 15 | — | 15 | 15 | 15, clippy's and not Sonar's |
| `parameter-count` | 4 | 6 (without `self` or `cls`) | 5 (without `@Override`) | 4 | 4 (receiver included, trait implementations excluded) |
| `nesting-depth` | 4 blocks | — | — | 4 control blocks | 4 blocks, `impl`, `trait` or modules |
| `short-name` | under 2 characters except `i`, `j` and `_`, properties excluded | `l`, `O` and `I` | locals under 3 characters except `i` and `j` | under 3 characters when the last use is more than 5 lines away | — |
| `magic-number` | outside module constants, enums, types and defaults; -1, 0, 1 and 2 are fine | only in comparisons; 0 and 1 are fine | outside fields, constants and `hashCode`; -1, 0, 1 and 2 are fine | in arguments, assignments, cases, conditions, operations and returns; 0 and 1 are fine | — |
| `duplicate-code` | identical functions of 3 lines or more and repeated branches of an `if` or a `switch`, within the file | blocks of 100 tokens repeated across files or within one | the same, not counting annotations, `import` or `;` | blocks of 150 syntax tree nodes repeated within a package | — |
| `commented-code` | — | comments on their own line that compile as Python | — | comment groups inside a function that compile as a Go statement | — |

Each language counts like its reference tool: ESLint and sonarjs, ruff, Checkstyle and PMD,
cyclop, gocognit, funlen, revive, mnd and varnamelen; duplicate blocks, PMD's CPD, dupl and
sonarjs (`no-identical-functions` and `no-duplicated-branches`); commented-out code, ruff's
ERA001 and gocritic's `commentedOutCode`; Rust, clippy. Those differences in criteria are not
documented anywhere; each one came out of comparing against real repos and is pinned in a test in
`src/metrics/tests.rs` (nesting, magic numbers and short names, in the `tests.rs` of their
module). Tests are not searched for magic numbers or duplicate blocks, TypeScript tests do not
get their function length measured and Python tests do not get their parameters counted. The
`minimal` preset does not look for magic numbers or duplicates, and `recommended` flags blocks
from 150 tokens (225 nodes in Go and 5-line functions in TypeScript).

In Rust, kerbo measures only what clippy measures (`too_many_lines`, `too_many_arguments`,
`cognitive_complexity` and `excessive_nesting`), and the cognitive complexity is clippy's, not
Sonar's: it starts at 1 and adds every `if`, every `match` with more than one arm and every guard
over the already expanded code, so `for`, `while`, every `?` and every `.await`, `assert!` and
`matches!` add up too. Then it discounts the `return`s: all of them if the function returns
`Result`, half otherwise. Closures are measured separately, including the ones inside a macro.
kerbo does not expand macros: it mimics the standard library's and `log`'s, and for the rest it
counts only the arguments; a crate's own macro that hides an `if` is the difference that remains.

Those are the thresholds of the `strict` preset. More rules and the layer for AI agents are in
the [roadmap](ROADMAP.md) (in Spanish).

## How it is built

The metrics are written once over the [tree-sitter](https://tree-sitter.github.io/) tree. What is
specific to each language lives in `src/profile/`: which node is a function, a decision or a
logical operator, and how the mimicked tool counts lines. Files are analyzed in parallel and one
at a time, without loading the whole program.

The CLI texts (report, `explain`, errors, hook, MCP and LSP) live in a typed catalog in
`src/i18n/`, one per language; the compiler makes sure every message exists in both.

## Development

The gate (formatting, strict clippy, size ceilings, tests with coverage and `cargo-deny`) is
`scripts/verify-rust.sh` and runs in a container, so all you need is Docker.

## License

[Apache 2.0](LICENSE).
