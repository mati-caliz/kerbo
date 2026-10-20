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

Linux and macOS, on x86_64 and ARM (on Linux, a static binary that runs on any distro), and
Windows x86_64 from Git's bash:

```sh
curl -fsSL https://kerbo.matiascaliz.com.ar/install.sh | sh
```

The installer downloads the latest version from the
[releases](https://github.com/mati-caliz/kerbo/releases) for the machine, verifies the sha256 and
leaves `kerbo` in `~/.local/bin` (another folder with `KERBO_INSTALL_DIR`).

With Homebrew or with Rust installed:

```sh
brew install mati-caliz/tap/kerbo
cargo install kerbo --locked
```

## Usage

```sh
kerbo check                       # the current directory
kerbo check api web               # several folders or files
kerbo check --language java api   # a single stack: typescript, python, java or go
kerbo check --diff main           # only what the lines changed since main touch
kerbo check --format sarif        # also json, agent, agent-json; GitHub and editors read sarif
kerbo check --no-cache            # analyze everything from scratch, without reading or writing the cache
kerbo baseline                    # accept what is already there: from now on only new problems
kerbo explain cognitive-complexity  # why a rule exists and how to fix it
```

It honors `.gitignore` and skips hidden files. It exits with `0` when there are no problems, `1`
when there are and `2` when something could not be read or the configuration is invalid.

`kerbo check`, `kerbo baseline` and the MCP server store what they measure of each file in a
content cache, so the next run only analyzes what changed (and what is compared across files, like
duplicate blocks, is recombined with what was stored). It lives in the user's cache folder
(`~/.cache/kerbo`, `~/Library/Caches/kerbo` or `%LOCALAPPDATA%\kerbo`; another one with
`KERBO_CACHE_DIR`), is shared across projects and clones, takes at most 256 MB and is discarded on
its own when the binary or the configuration that affects a file changes. If it breaks, it is
ignored. `--no-cache` turns it off.

Messages come out in Spanish or English. `--lang en` (or `es`) wins, then `lang` in
`kerbo.toml`, then the system locale (`LC_ALL`, `LC_MESSAGES`, `LANG`); without any of them,
Spanish. The language only changes the text meant for people: rule identifiers, stable `id`s and
the shape of the JSON, SARIF, `--format agent` and `agent-json` output stay the same.

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

- `kerbo check --format agent`: short, capped output, cheats first and then what exceeds the
  limit the most. It groups by file (the path appears once), each problem is a row whose columns
  are declared once (TOON style) with a short, stable `id`, and how to fix each rule appears once
  at the end. It spends 2.1 to 3.7 times fewer tokens than the previous JSON.
  `--format agent-json` gives the same thing as JSON, for code that parses it; the hook and the
  MCP server return `--format agent`.
  When the fix is mechanical (a redundant import, an extra modifier, a missing `final`), the
  `patch` section carries, by `id`, the unified diff hunk that solves it (in `agent-json`, the
  problem's `patch` field).
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
The ones with a mechanical fix show up as a quick fix: delete the import, drop the modifier or add
the `final`.

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
| `unused-import` | — | — | imports nobody names (the ones that only show up in Javadoc count as unused) and the ones from `java.lang`; fixed automatically | — | — |
| `redundant-import` | — | — | duplicated imports, from `java.lang` or from the same package; fixed automatically | — | — |
| `star-import` | — | — | `import a.b.*`, `static` ones included | — | — |
| `redundant-modifier` | — | — | what the language already assumes: `public`, `static`, `abstract` or `final` in an interface, `final` on a private method or in a final class, `static` on a nested enum or record, `public` on the constructor of a non-public class; fixed automatically | — | — |
| `final-parameter` | — | — | parameters of methods and constructors with a body that are not `final`; fixed automatically | — | — |
| `final-class` | — | — | classes whose constructors are all private (or private ones without a constructor) that are not `final` and are not extended in the file; fixed automatically | — | — |

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

The six Java form rules mimic Checkstyle 14 with `sun_checks.xml`, the reference configuration
Checkstyle ships and the one already used for `MagicNumber` and the lengths: `UnusedImports` (with
`processJavadoc` off, as in `sun_checks`), `RedundantImport`, `AvoidStarImport`,
`RedundantModifier`, `FinalParameters` and `FinalClass`. `google_checks.xml` only asks for
`AvoidStarImport`, and PMD 7's quickstart has the equivalents for imports, modifiers and final
classes but not `final` parameters. `recommended` does not ask for `final` parameters and `minimal`
only checks unused and duplicated imports. Columns count bytes, as in every other rule: with tabs,
Checkstyle expands them to 8 and reports a different number.

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

The cache (`src/cache/`) stores per file what the analysis measures, not the problems: the key is
the content, the language, the effective thresholds of that file and the identity of the binary
(version, size and date), and the path is left out, so a moved file or the same file in another
clone is not analyzed again. Rules and cross-file duplicates are always computed over what was
stored. It is split into 256 files by the first byte of the key, each with its own cap and
least-recently-used eviction; each entry is LZ4-compressed with its checksum, and it is written to
a temporary file that is renamed. In Rust the result of a file also depends on the crate's
`macro_rules!`, so the key adds a fingerprint of the text of every file in the crate and of the
file's path inside it: changing a macro analyzes again whoever uses it.

The CLI texts (report, `explain`, errors, hook, MCP and LSP) live in a typed catalog in
`src/i18n/`, one per language; the compiler makes sure every message exists in both.

A mechanical fix (`src/fix.rs`) is a list of range edits (start and end line and column and the
new text) plus the resulting unified diff hunk, computed during the analysis and stored in the
cache with the rest. The LSP sends the edits as a quick fix and `--format agent` sends the diff; a
new rule only has to build the `Fix` (delete a node or insert before one).

## Development

The gate (formatting, strict clippy, size ceilings, tests with coverage and `cargo-deny`) is
`scripts/verify-rust.sh` and runs in a container, so all you need is Docker.

## License

[Apache 2.0](LICENSE).
