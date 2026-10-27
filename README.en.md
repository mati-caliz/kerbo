# kerbo

[Español](README.md) · English

Code quality analyzer for **Java, TypeScript/JavaScript, Python, Go and Rust** in a single
binary. It applies the same rules to every language, in seconds and with a few megabytes of
memory, and counts exactly like the tool you already use for each one (ESLint, ruff, Checkstyle
and PMD, golangci-lint, clippy). It also checks shell scripts like ShellCheck, Postgres
migrations like squawk, Dockerfiles like hadolint and docker compose files.

```console
$ kerbo check --lang en
src/billing/InvoiceService.java:88:5: cyclomatic-complexity The function `settle` has cyclomatic complexity 11 (maximum 8)
web/src/orders/use-cart.ts:42:1: cognitive-complexity The function `applyPromotions` has cognitive complexity 19 (maximum 15)
kerbo: 5810 files, 2 problems, 2250 ms
```

## Installation

Linux and macOS, on x86_64 and ARM (on Linux, a static binary that runs on any distro), and
Windows from Git's bash:

```sh
curl -fsSL https://kerbo.matiascaliz.com.ar/install.sh | sh
```

On Windows, x86_64 or ARM, also from PowerShell:

```powershell
irm https://kerbo.matiascaliz.com.ar/install.ps1 | iex
```

The installer downloads the latest version from the
[releases](https://github.com/mati-caliz/kerbo/releases) for the machine, verifies the sha256 and
leaves `kerbo` in `~/.local/bin` (the PowerShell one, in `%LOCALAPPDATA%\Programs\kerbo`, and adds
it to `PATH`; another folder with `KERBO_INSTALL_DIR`).

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
kerbo check --osv                 # add the vulnerabilities of the dependencies (asks OSV)
kerbo osv-download                # download the OSV database to use --osv=offline without network
kerbo baseline                    # accept what is already there: from now on only new problems
kerbo fix                         # apply every mechanical fix at once
kerbo fix --dry-run --rule unused-import src  # only show the patch, for one rule and one folder
kerbo explain cognitive-complexity  # why a rule exists and how to fix it
```

It honors `.gitignore` and skips hidden files and folders, except `.github/` and `.devcontainer/`,
which usually hold the repo's own Dockerfiles, compose files and scripts; the other hidden ones
(`.env`, `.npmrc`) are only checked for secrets. It exits with `0` when there are no problems, `1`
when there are and `2` when something could not be read or the configuration is invalid.

`kerbo check`, `kerbo baseline` and the MCP server store what they measure of each file in a
content cache, so the next run only analyzes what changed (and what is compared across files, like
duplicate blocks, is recombined with what was stored). It lives in the user's cache folder
(`~/.cache/kerbo`, `~/Library/Caches/kerbo` or `%LOCALAPPDATA%\kerbo`; another one with
`KERBO_CACHE_DIR`), is shared across projects and clones, takes at most 256 MB (another cap with
`max_megabytes` in the `[cache]` section of `kerbo.toml`, or with `KERBO_CACHE_MAX_MB`, which wins
over the file) and is discarded on its own when the binary or the configuration that affects a
file changes. Two runs at once do not overwrite each other: each part of the cache is written under
an operating system lock, rereading what the other left. If it breaks, it is ignored. `--no-cache`
turns it off.

Messages come out in Spanish or English. `--lang en` (or `es`) wins, then `lang` in
`kerbo.toml`, then the system locale (`LC_ALL`, `LC_MESSAGES`, `LANG`); without any of them,
Spanish. The language only changes the text meant for people: rule identifiers, stable `id`s and
the shape of the JSON, SARIF, `--format agent` and `agent-json` output stay the same.

### Fixing everything at once

`kerbo fix` applies every mechanical fix (an import that is not needed, an extra modifier, a
missing `final`) in a single run. Among the fixes of a file it picks the ones that do not overlap
(two that touch the same range do not go together), applies them, analyzes the fixed text again
and repeats until none is left, so a fix that uncovers another one (a class that becomes `final`
makes the `final` of its methods redundant) also goes in; the ones that still overlap after ten
rounds are reported. Then it compares what is left with what was there: if a problem shows up that
was not there before, that file is not written and the new problems are listed. Each file is
written whole to a temporary file in the same folder that is then renamed, so it is never left
half written.

`--rule` (can be repeated) limits the fixes to those rules and the paths, to those folders or
files. `--dry-run` (or `--diff`) writes nothing and shows the patch as a unified diff without
context (`git apply --unidiff-zero`, also with CRLF). Like ruff, the fixes that may change what the
code does (removing an unused variable whose value has side effects, `x == True` becoming `x`) are
only applied with `--unsafe-fixes`; the LSP "fix all" skips them too. `--since <reference>` fixes only the problems
on lines changed since that git reference. A round of fixes that leaves new syntax errors is not
accepted. It exits with `0` if everything was written, `1` if some file was not written because the
fix left something new and `2` if something could not be read or written.

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

Everything is detected on the syntax tree: an `eslint-disable` inside a string does not count.
The comments of the tools kerbo replaces count as a new suppression too: `-- squawk-ignore` and
`-- squawk-ignore-file` in migrations, `# hadolint ignore=` and `# hadolint global ignore=` in
Dockerfiles, and `# shellcheck disable=`. If an exception is legitimate, it goes in `kerbo.toml`
with its `reason`, in plain sight.

Those comments are also honored when reporting, the way the original tool does it: the problem of
the rules named on the next line (or in the whole file, with `-file` and `global`) is not counted.
What follows the list of rules is the reason
(`# hadolint ignore=DL3008 # the base image pins the versions`), and every ignored problem shows up
in the log with its reason, or `no reason given`, just like the deviations in `kerbo.toml`; the
summary adds how many were ignored in the code. `squawk-ignore` works, as in squawk, for its own
line and the next one (also at the end of a line or inside `/* */`, with the reason after another
`--`), and Go's `//nolint` with golangci-lint's scope. With `--format sarif` whatever is ignored
comes out as suppressed in the source (`inSource`), with its reason, and GitHub shows it as
dismissed.

### For AI agents

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

### In the editor

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
language = "java"                 # optional: typescript, python, java, go, rust, sql or docker
reason = "the team agreed on 12 for the services"

[[exceptions]]                    # per folder or file
paths = ["src/legacy/**"]
rule = "cognitive-complexity"     # without `max`, the rule does not run there; without `rule`, none does
reason = "it gets rewritten in the first quarter"

[[ignore]]
paths = ["generated/**"]
reason = "generated by the protobuf compiler"

[cache]
max_megabytes = 512               # optional: the cache cap, 256 by default

[rust]                            # optional: what the #[cfg]s are evaluated with
target_os = "windows"             # by default, the machine kerbo runs on
target_arch = "aarch64"
features = ["std"]                # by default, all of them (like --all-features)
cfg = ["fast_path"]               # the ones the build.rs turns on

[go]                              # optional: which Go files count, like go/build
goos = "linux"                    # by default, GOOS or the machine kerbo runs on
goarch = "amd64"                  # by default, GOARCH or the machine
build_tags = ["integration"]      # like golangci-lint's run.build-tags
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

From the rest of `sun_checks.xml`, kerbo does the names (`TypeName`, `MethodName`, `ConstantName`,
`StaticVariableName`, `MemberName` and `ParameterName`, with the same rule names in kebab case:
`type-name`, `method-name`...), `UpperEll` (`upper-ell`, fixed automatically), `EmptyStatement`
(`empty-statement`), `ModifierOrder`, `ArrayTypeStyle`, `MultipleVariableDeclarations`,
`InterfaceIsType`, `MissingSwitchDefault`, `NeedBraces`, `EmptyBlock`, `AvoidNestedBlocks`,
`InnerAssignment`, `SimplifyBooleanExpression`, `EqualsHashCode`, `HideUtilityClassConstructor`,
`TodoComment`, `IllegalImport`, `FileTabCharacter`, trailing spaces (`trailing-spaces`, the
`RegexpSingleline` in `sun_checks`), `NewlineAtEndOfFile`, `DesignForExtension`, `HiddenField`,
`VisibilityModifier`, `DeclarationOrder`, `LocalFinalVariableName` and the whitespace and brace
ones: `WhitespaceAround`, `WhitespaceAfter`, `NoWhitespaceBefore`, `NoWhitespaceAfter`,
`GenericWhitespace`, `MethodParamPad`, `EmptyForIteratorPad`, `LeftCurly`, `RightCurly` and
`OperatorWrap`. Trailing spaces, `NewlineAtEndOfFile` and the whitespace ones are fixed
automatically (not the braces nor `OperatorWrap`, which need lines rearranged). They follow
Checkstyle where it is not obvious: `MethodName` skips what carries `@Override` and flags the
method named like its class; an interface field is a constant; `serialVersionUID` is left out;
the patterns are ASCII, so an `ñ` in a name is flagged; `WhitespaceAround` also flags empty braces
written together (`{}`), as `sun_checks` does. `minimal` does not ask for them.

From IntelliJ's inspections, kerbo does:

- `SameParameterValue` (`same-parameter-value`): a parameter that every call passes the same
  constant, shown the way IntelliJ shows it (`"USD"`, `Prices.SCALE`). Private code is checked
  per file; package-private and protected code, over the whole project (IntelliJ does not flag
  public code).
- `UnusedReturnValue` (`unused-return-value`): a method that returns something no call uses.
- `SameReturnValue` (`same-return-value`): every `return` gives back the same literal. For an
  abstract method, all its implementations (anonymous classes too); at least one has to be outside
  tests, because a Spring Data projection only has test stubs and Spring builds the real one. An
  interface with a single abstract method is skipped: it may have lambda implementations that
  cannot be seen.
- `unused` for methods (`unused-method`): no call, reference (`::name`) or string (`"name"`, as in
  `@MethodSource`) in the project names it, and no known framework calls it. Frameworks are data
  tables in `src/index/java_unused.rs`: a method with an annotation outside the list of passive
  ones (`@Transactional`, `@Deprecated`, `@Nullable`…) counts as used (so `@GetMapping`, `@Bean`,
  `@PrePersist`, `@Test`, `@JsonCreator` or a custom annotation do), as do the methods of a
  MapStruct `@Mapper` or an `@Aspect`, accessors (`get`/`is`/`set`, used by Jackson and JPA) and
  the ones the JDK calls by name. Public and protected code is only flagged in an application
  (`@SpringBootApplication`, `@QuarkusMain`, `@MicronautApplication`): in a library it is its API.
- `unused` for fields that are assigned but never read (`unread-field`): a private field the code in
  its file only writes. A parameter or local variable with the same name does not read it
  (`this.total = total`); a `+=` or a `++` does. The framework tables are the ones of
  `unused-field`.
- `unused` for fields (`unused-field`): no code or string in the project names the field (private
  fields are checked in their own file). A string that initializes a constant of the same name
  (`PAID = "PAID"`) does not count as a use. Injection annotations (`@Autowired`, `@Inject`,
  `@Value`…) do not save a field nobody reads; any other non-passive one does (`@Column`, `@Mock`,
  `@MockitoBean`, `@JsonProperty`), and so does a class annotation that may read its instance
  fields (`@Data`, `@Getter`, `@Entity`, a custom one). Spring stereotypes, test annotations and
  Lombok constructors do not read fields: an unused `private final` in a `@Service` with
  `@RequiredArgsConstructor` is flagged.

- `NullableProblems` (`nullable-problems`): an `@Override` method (also in an anonymous class)
  overriding one from a library that is non-null by default (Spring 7 with `@NullMarked`, Spring 6
  with `@NonNullApi`) that leaves a parameter or the returned object unannotated. Like IntelliJ,
  it does not flag a parameter the body dereferences before any exiting `if` or a `try` (from
  that it infers non-null), what the library declares `@Nullable`, or anything if the project
  package is already `@NullMarked`; the return is flagged even if it returns a literal.

- `DataFlowIssue`, the chain case (`data-flow-issue`): a call on the result of a method its
  library declares `@Nullable`, in the same expression (`response.getBody().error()`,
  `SecurityContextHolder.getContext().getAuthentication().getName()`). Each step's type comes from
  the previous one's signature in the jars. It is not flagged if that same result was checked
  earlier in the method (`!= null`, `assertThat(...).isNotNull()`, `assertNotNull`,
  `requireNonNull`). It also follows variables: one assigned from a `@Nullable` call and used on a
  path without a check (`if (x == null) return;`, `x != null && ...`, `instanceof`, `hasText`,
  loops, `try`, its resources and `finally` are understood as IntelliJ does), and a `@Nullable`
  result that gets unboxed (`return jdbc.queryForObject(...)` in a method returning `long`). With
  overloads, a `Long.class` argument picks the ones taking a `Class`, as the compiler does; a
  varargs method accepts its parameter count or more. For the JDK, which does not come in the jars,
  there is kerbo's own table (`src/index/java_jdk.rs`) of what IntelliJ treats as possibly null:
  `getResourceAsStream` and `getResource`, `System.getProperty` and `getenv`, `File.getParentFile`,
  `getParent`, `listFiles` and `list`. That part works without a classpath.
- `OptionalGetWithoutIsPresent` (`optional-get-without-is-present`): `get()` chained to
  `findFirst()`, `findAny()`, `max(...)`, `min(...)` or `Optional.ofNullable(...)`.

- `AutoCloseableResource` (`auto-closeable-resource`): a call that creates something closeable (an
  `InputStream`, a `Connection`, a `ResultSet`, an `ExecutorService`, an `HttpClient`...) outside a
  try-with-resources: in a variable that is not closed, returned or passed to another method, or
  chained (`factory().getValidator()`). What is closeable comes from the jars, from the type a
  project method declares, or from kerbo's own JDK table; what a `get...` returns belongs to whoever
  gives it (the container or the pool).

And for security, what reaches a query (`sql-injection`), a command (`command-injection`), a file
path (`path-traversal`) or the HTML response (`xss`) from the user unsanitized. Sources are the `HttpServletRequest`
methods that return what the user sent (`getParameter`, `getHeader`, `getCookies`,
`getQueryString`...) and parameters with `@RequestParam`, `@PathVariable`, `@RequestBody` and the
other Spring web annotations. Sinks are `Statement`, `Connection`, `JdbcTemplate`,
`EntityManager` and `JdbcClient` (in a query only the text counts: `?` arguments are safe),
`Runtime.exec` and `ProcessBuilder`, and `File`, `FileInputStream`, `FileOutputStream`,
`FileReader`, `FileWriter`, `RandomAccessFile`, `Paths.get` and `Path.of`, and what is written
through `response.getWriter()` or `getOutputStream()` (chained or kept in a variable). An HTML
escape (ESAPI's `encodeForHTML`, `HtmlUtils.htmlEscape`, `escapeHtml4`, `Encode.forHtml`...)
protects the response but not a query or a path: escaped data is followed separately. All of it is
a data table in `src/lint/java_taint/tables.rs`.

Each method is walked like an interpreter: it knows which variables are constants and uses them to
decide `if`, `switch` and ternaries (`(7 * 42) - 86 > 200 ? "fixed" : data` always takes the same
branch), and it follows the lists it builds position by position and its maps key by key (taking
`list.get(1)` when the data went into slot 0 is not tainted). Across methods and files, each one
leaves a summary (what it returns depending on its parameters and what reaches each sink) and the
index combines them: a call goes to the methods of its type, to the ones it inherits and to its
implementations; a type in the same file shadows same-named ones elsewhere. What an abstract
method of an interface nobody implements in the code returns (a Spring Data repository) comes from
the database, not from its arguments. A library call returns what its receiver and arguments
bring, except the ones that give a number, a boolean or a UUID (`parseInt`, `length`, `equals`,
`UUID.fromString`...).

In Python, the same four rules with the same engine. Sources are Flask's `request.args`, `.form`,
`.cookies`, `.headers`, `.values`, `.files`, `.json`, `.get_json()` and friends, Django's (`GET`,
`POST`, `COOKIES`, `META`, `body`) and Pyramid's (`params`, `matchdict`), the parameters of a route
function (`@app.route`, `@router.get`...) and the `event` of a Lambda handler. Sinks: `.execute`,
`.executemany` and `.executescript`, `text()` and the SQLAlchemy `Query` methods that take SQL as
text; `os.system`, `os.popen` and `os.exec*`/`spawn*` always, and `subprocess.*` only with
`shell=True` or when what it runs is a shell with `-c` (without a shell, a built string is not
interpreted); `open`, `codecs.open`, `os.remove`, `os.path.exists`, `shutil.*`, `send_file` and the
methods of a `pathlib` object; and the response body: what a Flask view returns (not when it
returns a dict or uses `jsonify`; in FastAPI it is JSON), `make_response` and
`render_template_string`. An `if '..' in path: return` validates the path; an HTML escape
(`html.escape`, `markupsafe.escape`, `render_template` or a project function named "escape…html"),
`shlex.quote` and `secure_filename` clean for their sink; what builds a SQLAlchemy statement
(`select`, `insert(...).values(...)`) uses bound parameters. Across modules, calls are resolved
through imports (relative ones and the ones inside a function too), the functions and classes
visible from each one, `self` and the class a variable was created with. `os.name` is taken as
`"posix"`: that is where the server runs.

In JavaScript and TypeScript, the same four rules. Without types, sources and sinks are recognized
by name: the request (`req`, `request`, `ctx`) and its `query`, `params`, `body`, `cookies`,
`headers`, `url` and friends; the `event` of a Lambda handler (`(event, context)`, or a lone
`event` in a function called `handler`); and in the browser, `location.hash`, `location.search`,
`document.URL`, `document.referrer` and `window.name`. Sinks: `query`, `execute`, `raw` and
`$queryRawUnsafe` on a connection (`db`, `conn`, `pool`, `client`, `knex`, `prisma`...);
`child_process.exec` always and `spawn`/`execFile` with `{ shell: true }` or with a program chosen by the user; `fs.*` and
`res.sendFile`; and for HTML, `res.send`, `res.write` and `res.end` from Express and Node (not
Fastify's, which sends plain text, nor when the function declares another type with
`res.type('json')` or `writeHead(200, { 'content-type': 'application/json' })`), `innerHTML`,
`document.write`, `insertAdjacentHTML`, `dangerouslySetInnerHTML`, jQuery (`$("<b>" + x)`,
`.html(x)`) and the response of a Lambda handler declared as `text/html`. `DOMPurify.sanitize`,
`sanitize-html`, `escape-html`, `$sanitize` or a project function named "escape…html" clean HTML;
`path.basename`, the path. Every function (anonymous route handlers too) is a method for the
engine, and across files `import` and `require` are resolved, relative ones too.

In Go, the same four rules, and since Go declares types, sources and sinks come from them: a
`*http.Request` parameter or a Lambda event (`events.APIGatewayProxyRequest`) brings everything
from the user, and from a `*gin.Context` or an `echo.Context` only what reads the request
(`Query`, `Param`, `PostForm`...). Sinks: `Query`, `QueryRow`, `Exec` and `Prepare` (and their
`...Context` variants, with the query after `ctx`) on `*sql.DB`, `sqlx` and `pgx`, and gorm's
`Raw`, `Where`, `Order` and friends, following the type along the chain
(`db.Table("x").Order(field)`); `exec.Command` with a shell and `-c`; `os.Open`, `os.ReadFile`,
`os.Remove` and friends and `http.ServeFile`; and `w.Write`, `fmt.Fprintf(w, ...)`,
`io.WriteString(w, ...)` on an `http.ResponseWriter` (not when the function sets a `Content-Type`
that is not HTML), `template.HTML` and a Lambda response declared as `text/html`.
`html.EscapeString`, `template.HTMLEscapeString` and `json.Marshal` (it escapes `<` and `>`) clean
HTML; `filepath.Base` and `path.Clean("/" + path)`, the path; `strconv.Atoi` and friends give a
number. What is written into a `strings.Builder` stays in it. Anonymous functions (the `func(w, r)`
of a `HandleFunc`) see the types of the variables of the functions that contain them, and across
packages calls are resolved by directory: the import path ends like the package's directory.

To read libraries, kerbo asks for the classpath once, offline, to Maven
(`mvn -o dependency:build-classpath`) or to Gradle (the project's `gradlew` when there is one, with
kerbo's own init script that lists the jars of `testRuntimeClasspath`, `runtimeClasspath` and
`compileClasspath` and skips what is not downloaded), and stores it with the fingerprint of the
build files; from the jars it reads only the index and the classes it needs, no bytecode. With any
other tool, the list of jars goes in `KERBO_JAVA_CLASSPATH` (separated as in PATH). Without a
classpath, the rules that read libraries only know kerbo's own JDK table.

The ones that look at the whole project only run when the analysis covers it (`kerbo check` at the
root): over a folder or a single file, calls would be missing. Without compiling, kerbo resolves a
call's receiver when it can be read: `repo.find()` with `Repository repo` declared in the file,
`Type.method()`, or `method()` from inside a class (with what it inherits through `extends` and
`implements`). A call chain, an untyped lambda or a `static import` stay unknown and count for
every candidate; with two equal methods in the same class or in related classes nothing is
flagged. In a library (without `@SpringBootApplication` or similar), public and protected code
is API and who calls it is not checked.

In Rust, kerbo measures only what clippy measures (`too_many_lines`, `too_many_arguments`,
`cognitive_complexity` and `excessive_nesting`), and the cognitive complexity is clippy's, not
Sonar's: it starts at 1 and adds every `if`, every `match` with more than one arm and every guard
over the already expanded code, so `for`, `while`, every `?` and every `.await`, `assert!` and
`matches!` add up too. Then it discounts the `return`s: all of them if the function returns
`Result`, half otherwise. Closures are measured separately, including the ones inside a macro.
The crate's own `macro_rules!` are expanded like rustc does, reading the definitions from all its
files, and so are the ones exported by its dependencies (`dep::macro!`, `use dep::macro` or
`#[macro_use] extern crate`), read from cargo's cache with the `Cargo.lock` version or, without a
lockfile, the newest one compatible with the requirement. The standard library's are mimicked; for
procedural macros only the arguments count.

Since clippy measures what compiles, an item, a statement or a `match` arm under a false `#[cfg]`
does not count for complexity or nesting, and a file with a false `#![cfg(..)]` is not measured.
The `#[cfg]`s are evaluated with the machine kerbo runs on, every feature in `Cargo.toml` and
`test` in both modes (the library and its tests); what kerbo does not know, like a `cfg` the
`build.rs` emits, decides nothing and the code is measured. The `[rust]` section of `kerbo.toml`
changes the platform, the features and the enabled `cfg`s.

### Python against ruff

In Python, kerbo implements ruff's classic rule set (`E4`, `E7`, `E9` and `F`: pyflakes and the
pycodestyle errors) under ruff's own codes, lowercase: `kerbo explain f401`, `--max f841=0` or
`f401 = "off"` in `kerbo.toml`. It honors `# noqa`, `# noqa: F401` and `# ruff: noqa` like ruff
(in multi-line strings and `\` continuations the `noqa` goes on the last line). `minimal` keeps
only the `F` rules.

| Rules | What they flag |
|---|---|
| `f401` | unused imports (fixed automatically, except in `__init__.py`) |
| `f811`, `f402` | redefinitions of something never used and `for` variables that shadow an import |
| `f821`, `f405`, `f822`, `f823` | undefined names (with module, class, function and comprehension scopes, quoted annotations and `from __future__ import annotations`), names that may come from an `import *`, names in `__all__` and locals used before assignment |
| `f841`, `f842` | local variables assigned or annotated and never used (an `except` `as` is fixed automatically) |
| `f403`, `f404`, `f406` | `import *`, a late `from __future__` and `import *` outside the module level |
| `f501`–`f509`, `f521`–`f525` | `%` and `.format()` formats that do not match their arguments |
| `f541` | f-strings without placeholders (fixed automatically) |
| `f601`, `f602` | repeated dictionary keys |
| `f631`, `f632`, `f633`, `f634` | `assert` and `if` on a tuple, `is` against a literal (fixed automatically) and `print >>` |
| `f701`, `f702`, `f704`, `f706`, `f707`, `f722`, `f901` | misplaced `break`, `continue`, `yield`, `await` and `return`, `except:` before other handlers, quoted annotations that do not parse and `raise NotImplemented` (fixed automatically) |
| `e401`, `e402` | several modules in one `import` and imports after other code |
| `e701`, `e702`, `e703` | several statements on one line (the trailing `;` is fixed automatically) |
| `e711`, `e712`, `e713`, `e714`, `e721` | `== None`, `== True`, `not x in y`, `not x is y` (fixed automatically) and type comparisons with `==` |
| `e722`, `e731`, `e741`, `e742`, `e743` | bare `except:`, assigned lambdas and the names `l`, `I` and `O` |

Parity, against ruff 0.17 with `--select E4,E7,E9,F`: zero differences by file, line, column and
rule on django, fastapi, requests, flask, pydantic, sqlalchemy, ipython, ansible, sympy and the
CPython standard library (about 11,000 files and 15,900 findings). The only ones left are in files
tree-sitter-python cannot parse (there kerbo skips the scope rules) and E902, which is a read error
rather than a rule.

### TypeScript and JavaScript against ESLint

In TypeScript and JavaScript, kerbo runs the most frequent rules of `@eslint/js` recommended and
of `typescript-eslint` recommended that need no type information, with the ESLint name without the
plugin prefix: `kerbo explain no-unused-vars`, `--max no-explicit-any=0` or `no-var = "off"`. They
run in both languages as in an `eslint.config.js` with `js.configs.recommended` and
`tseslint.configs.recommended`; in `.ts` files `no-dupe-keys` does not run, since the compiler
already checks it. `no-var` and `prefer-const` also run in JavaScript and `no-non-null-assertion`
(from `strict`) is part of the `strict` preset. `recommended` turns it off and `minimal` keeps only
the ones ESLint classifies as problems. `eslint-disable` silences nothing: the problem is reported
and the anti-cheat flags the comment.

| Rules | What they flag |
|---|---|
| `no-unused-vars` | unused variables, functions, classes, types, imports and parameters, with the scopes of `@typescript-eslint/scope-manager` (types and values apart, `typeof`, JSX against `React`, ambient declarations and `.d.ts`) |
| `prefer-const`, `no-var` | a `let` that is never reassigned and `var`; both fix themselves when ESLint fixes them |
| `no-unused-expressions` | stray expressions that do nothing |
| `no-require-imports` | `require()` without a local `require` and `import x = require()` |
| `no-explicit-any`, `no-non-null-assertion`, `ban-ts-comment` | `any`, the non-null `!` and `@ts-ignore`, `@ts-nocheck` or `@ts-expect-error` without a reason |
| `no-useless-escape` | unneeded escapes in strings, templates and regular expressions (fixes itself) |
| `prefer-rest-params`, `no-prototype-builtins`, `no-this-alias`, `no-empty-object-type` | `arguments`, `obj.hasOwnProperty()`, `const self = this` and the `{}` type |
| `no-empty`, `no-cond-assign`, `no-case-declarations` | empty blocks, assignments in conditions and loose declarations in a `case` |
| `no-debugger`, `no-duplicate-case`, `no-dupe-else-if`, `no-dupe-keys` | `debugger` (fixes itself), repeated cases, branches and keys |

Parity, against ESLint 10.13 and typescript-eslint 8.72: no extra difference by file, line, column
and rule in next.js, react, VS Code, zod and express (42,488 problems in about 30,000 files). 587
problems are missing, all from files tree-sitter does not understand (`in`/`out` variance,
`let as`, `export type *`, escapes in template types) or from `.js` files with type arguments in
calls, which TypeScript reads as comparisons.

### Shell against ShellCheck

`.sh` and `.bash` files and extensionless files with an `sh`, `bash`, `dash` or busybox shebang
are checked against [ShellCheck](https://www.shellcheck.net/), with its code as the rule name,
lowercase: `kerbo explain SC2086`, `--max sc2086=0` or `sc2034 = "off"` in `kerbo.toml`. These are
the 22 rules that show up most; severity is ShellCheck's, `recommended` turns off the style ones
and `minimal` the informational ones too. The universal rules in the table above do not run on
shell, except for the anti-cheat.

| Rules | What it flags |
|---|---|
| `sc2086` | unquoted expansions that may split or glob, following each variable's value through branches, loops and functions like ShellCheck (fixes itself) |
| `sc2046`, `sc2206`, `sc2207`, `sc2068` | unquoted `$(…)` and `${arr[@]}`, and arrays built from words that split |
| `sc2006`, `sc2005`, `sc2116` | backticks (fixes itself), `echo $(cmd)` and `$(echo …)` |
| `sc2034` | variables assigned and never used |
| `sc2155` | `local x=$(cmd)`, which hides the exit code |
| `sc2162`, `sc2164` | `read` without `-r` and `cd` without `\|\| exit` (both fix themselves) |
| `sc2004`, `sc2181`, `sc2015`, `sc2166` | needless `$` in arithmetic, `[ $? -ne 0 ]`, `a && b \|\| c` and `-a`/`-o` inside `[ ]` |
| `sc2016`, `sc2059`, `sc2064`, `sc2069`, `sc2223` | expansions in single quotes, variables in a `printf` format, `trap` with double quotes, redirections in the wrong order and unquoted `: ${x:=…}` |
| `sc2148` | a script with no shebang and no `# shellcheck shell=` |

`# shellcheck disable=` works as in ShellCheck: before the first command, for the whole file;
before a compound command, for the whole block; it accepts ranges (`SC2000-SC2099`) and `all`.

Parity, against ShellCheck 0.11: zero differences by file, line, column and rule on 1,752 scripts
from kubernetes, moby, nvm, rustup, ohmyzsh, acme.sh, bash-it, dokku and others (1,680 findings).
Files tree-sitter-bash cannot parse are skipped.

### Go against golangci-lint

In Go, kerbo runs rules from golangci-lint's default set under the same names:
`kerbo explain errcheck`, `--max ineffassign=0` or `qf1003 = "off"` in `kerbo.toml`. `errcheck`
reports the `error` a call returns that nobody checks, with errcheck's exclusions; the types come
from Go's own type checker through a helper kerbo builds once with the system's `go` (without
`go`, the scan fails with the reason, or it is turned off with `errcheck = "off"`). `ineffassign` reports an assignment nobody reads before it is overwritten or the
function ends, following branches, loops, `switch` with `fallthrough`, `select`, `goto` and labels;
variables captured by a closure or whose address is taken do not count. From staticcheck: `st1005`,
error messages with a leading capital or trailing punctuation (with the whole package's constants
and the same exemptions), `qf1003`, `if` chains that always compare the same thing and could be a
`switch`, and `qf1004`, `strings.Replace` and friends with a `-1` limit.

Like golangci-lint, it skips generated files, files that do not build on the platform (suffixes
like `_windows.go`, `//go:build` and `// +build`) and whatever `./...` does not list from the
`go.mod` (`testdata`, `vendor` and folders starting with `_` or `.`). The platform is the one in
`GOOS` and `GOARCH` or the machine kerbo runs on; the `[go]` section of `kerbo.toml` pins it and
adds build tags.

`//nolint` works as in golangci-lint: at the end of a line, for that line; on its own line, for
whatever starts right below at the same column (a function, an `if`, the whole file when it sits
before `package`). Without a list or with `all` it silences every rule, and the reason in
`//nolint:ineffassign // reason` shows up in the report.

Parity, against golangci-lint 2.15 on linux/amd64: zero differences by file, line and column in
golang/tools, terraform, hugo, caddy, consul, prometheus, etcd, syncthing and others (1,381
reports), in errcheck 10,699, in `qf1008` and `qf1012` 1,153 and in `sa1019` (use of deprecated
APIs) 592, `qf1001` 136, govet's `stdmethods` 28 and the small staticcheck tail (`st1012`, `qf1011`,
`st1023`, `qf1002`, `s1005`, `sa9003`, `qf1006`, `st1019`, `sa1029`, `sa9004`, `qf1009`, `st1013`,
`s1000`, `st1017`, `st1006`, `s1008` and `s1002`) 180 and in `unused` (code nothing uses) 249, with
none extra: 99 % of what golangci-lint reports by default. The rest of govet and staticcheck
stays in golangci-lint.

### Secrets and vulnerable dependencies

`committed-secret` checks every committed file, not just code, with the 222 rules of gitleaks'
default configuration (AWS, GCP and Azure keys, GitHub, GitLab, Slack and Stripe tokens, private
keys, JWTs…), its allowlists, its entropy threshold and its decoding of base64, hex, URL and
Unicode escapes, plus a rule of its own for passwords in connection strings. It reports the same
as `gitleaks dir` on the repos where it was measured. The secret is never printed whole nor stored
in the cache: only its first four characters and its length. A `gitleaks:allow` on the line lets
it through; test data is excluded in `kerbo.toml` with `[[exceptions]]`,
`rule = "committed-secret"` and its `reason` (`.gitleaksignore` files are not read, so nothing is
loosened silently).

`vulnerable-dependency` is the only rule that uses the network, and only with `--osv`: it reads
`Cargo.lock`, `package-lock.json`, `pnpm-lock.yaml`, `yarn.lock`, `go.mod`, `go.sum`,
`poetry.lock`, `uv.lock`, `Pipfile.lock`, `requirements*.txt` pinned with `==`, `pom.xml` (with its
local parents and their properties) and `gradle.lockfile`, and asks OSV about the exact versions.
It talks to `api.osv.dev` through the system's `curl`, with a one-minute limit per request, and
keeps the answers in the cache for a day; advisories are kept until OSV modifies them.
`kerbo osv-download` downloads each ecosystem's database (about 280 MB in all, almost all of it
npm) and `--osv=offline` uses it without network. Each problem names the advisory, its aliases and
the version that fixes it. Both rules are in all three presets, tests included.

Those are the thresholds of the `strict` preset. More rules and the layer for AI agents are in
the [roadmap](ROADMAP.md) (in Spanish).

## Migrations and containers

kerbo also checks what ships next to the code: Postgres migrations, Dockerfiles and docker compose
files.

**Migrations.** Migrations are the `.sql` files inside a `migrations`, `migration` or `drizzle`
folder (Prisma, drizzle, sqlx, golang-migrate) and the ones named like Flyway
(`V3__add_index.sql`, `R__views.sql`); other `.sql` files (seeds, dumps) are left alone. For
Alembic, kerbo reads the `upgrade()` of each revision: every `op.*` and `batch_op.*` becomes the
SQL Alembic would generate for Postgres (SQLAlchemy types, `server_default`, foreign keys, the
integer primary key as `SERIAL`), together with the SQL in `op.execute(...)`, and all of it goes
through the same rules as a `.sql` file, reported at the call's line. `for` loops over lists,
helper functions in the same file and module constants are followed, `autocommit_block()` leaves
the transaction, and branches for other dialects (`if dialect == "mysql"`) and `except` blocks are
skipped. The rules are
[squawk](https://squawk.sh)'s, with its names and criteria, assuming like `--assume-in-transaction`
that each file runs in a transaction (Prisma, drizzle, Flyway and Alembic do on Postgres): an index
on a table created in the same migration is not flagged.

| Rule | What it flags |
|---|---|
| `require-concurrent-index-creation` | `CREATE INDEX` without `CONCURRENTLY` on a table that already existed |
| `require-concurrent-index-deletion` | `DROP INDEX` without `CONCURRENTLY` |
| `adding-required-field` | `ADD COLUMN ... NOT NULL` without a `DEFAULT` or a generated column |
| `adding-not-nullable-field` | `SET NOT NULL` without a `CHECK (column IS NOT NULL) NOT VALID` validated first |
| `changing-column-type` | `ALTER COLUMN ... TYPE` (also on composite types) |
| `renaming-column`, `renaming-table` | renamed columns, attributes and tables |
| `ban-drop-column`, `ban-drop-table` | dropped columns, attributes and tables |
| `require-lock-timeout`, `require-statement-timeout` | the first lock-taking operation without a `SET lock_timeout` or `statement_timeout` before it |
| `constraint-missing-not-valid`, `adding-foreign-key-constraint` | constraints and foreign keys without `NOT VALID`, or validated in the same transaction |
| `adding-serial-primary-key-field`, `disallowed-unique-constraint` | `PRIMARY KEY` and `UNIQUE` without `USING INDEX` on a table that already existed |
| `adding-field-with-default` | a column with a volatile default (`random()`, `gen_random_uuid()`) or a generated one |
| `prefer-bigint-over-int`, `prefer-bigint-over-smallint`, `prefer-identity`, `prefer-text-field`, `prefer-timestamp-tz`, `ban-char-field` | `int`, `smallint`, `serial`, `varchar(n)`, `timestamp` without a time zone and `char(n)` in new or changed columns |
| `ban-drop-constraint`, `ban-drop-default`, `ban-drop-not-null`, `ban-drop-generated-expression` | dropped constraints, defaults, `NOT NULL` and generated expressions |
| `ban-drop-type`, `ban-drop-function`, `ban-drop-sequence`, `ban-drop-view`, `ban-drop-schema`, `ban-drop-domain`, `ban-drop-extension`, `ban-drop-database` | dropped objects the code may use |
| `renaming-object`, `ban-set-schema` | renamed constraints, indexes, types, views and functions, and objects moved to another schema |
| `require-enum-value-ordering`, `ban-alter-identity`, `ban-create-domain-with-constraint`, `ban-alter-domain-with-add-constraint` | enum values without a position, changed identities and domains with constraints |
| `require-concurrent-reindex`, `require-concurrent-partition-detach`, `ban-truncate-cascade` | `REINDEX` and `DETACH PARTITION` without `CONCURRENTLY`, and `TRUNCATE ... CASCADE` |
| `transaction-nesting`, `ban-uncommitted-transaction`, `ban-concurrent-index-creation-in-transaction`, `prefer-robust-stmts` | `BEGIN`/`COMMIT` inside the tool's transaction, unclosed transactions, `CONCURRENTLY` inside a transaction and statements without `IF [NOT] EXISTS` outside one |
| `identifier-too-long`, `ban-duplicate-column-assignments` | names longer than 63 bytes and repeated columns in a table, an `INSERT`, a `SET` or a view |

It is squawk 2.70's whole default set except `prefer-repack`, which squawk only runs for
Postgres 19. `recommended` turns off the type preferences (`prefer-*` and `ban-char-field`) and
`minimal` also the timeouts. Parity, against squawk 2.70 with `--assume-in-transaction`: all 9,612
of squawk's reports, none extra, in 1,858 migrations from lobe-chat (drizzle), documenso,
trigger.dev and cal.com (Prisma), conductor (Flyway) and awesome-compose. In Alembic, against squawk
on the SQL Alembic itself generates in offline mode (300 revisions from airflow, superset, mlflow
and redash; the other 349 import the application or query the database and cannot be converted
without running it): 297 identical by rule and count, none with fewer reports, and two with extra
reports on paths offline mode does not take (`is_offline_mode()` and a `try` that breaks offline).

**Dockerfile.** `Dockerfile`, `Containerfile`, `Dockerfile.*` and `*.Dockerfile`, against
[hadolint](https://github.com/hadolint/hadolint): every DL rule hadolint ships turned on (59),
named by their code (`kerbo explain DL3008`). The shell of each `RUN` goes through an in-house
parser that splits commands like ShellCheck. Left out are the ShellCheck rules (`SC*`), which come
with stage 10, and the ones hadolint ships turned off or only runs with its configuration (DL1001,
DL3026, DL3049 to DL3058). `.hadolint.yaml` is not read: a rule is turned off in `kerbo.toml`, with
its reason. `recommended` turns off hadolint's `info` level and `minimal` keeps only its `error`
level. Parity, against hadolint 2.15.1 with its default configuration: all 408 of hadolint's
reports, none extra, in 169 Dockerfiles from awesome-compose, docker-stacks, airflow, superset,
mlflow, nextcloud/docker, mastodon, sentry self-hosted, grafana and redash.

**docker compose.** `compose.yaml`, `docker-compose.yml` and their variants
(`docker-compose.prod.yml`), with anchors and `<<:` resolved. There is no reference tool: the
criteria are kerbo's own.

| Rule | What it flags |
|---|---|
| `compose-memory-limit` | a service with `image` or `build` and neither `deploy.resources.limits.memory` nor `mem_limit`; services using `extends` and override fragments are skipped |
| `compose-healthcheck-start-period` | an active `healthcheck` without `start_period` |
| `compose-healthcheck-interval` | an `interval` under 5 s or over 5 min, or a `timeout` equal to or longer than the interval |
| `compose-cpu-limit` | a service with `image` or `build` and no `deploy.resources.limits.cpus` nor `cpus` |
| `compose-restart` | a service without `restart` nor `deploy.restart_policy` (`restart: "no"` is fine for a one-off task) |
| `compose-image-tag` | an image without a tag or with `latest`; a digest or a variable does not count |
| `compose-depends-on-healthy` | a `depends_on` on a service with a healthcheck that does not ask for `condition: service_healthy` |

Each one is turned off like any other rule, with a `[[limits]]` entry without `max` and its reason;
`max` does not apply. `--language sql` or `--language docker` analyzes only those.

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
`macro_rules!`, so the key adds a fingerprint of the crate files that define macros, bring them in
with `#[macro_use]` or invoke one that writes functions, of each dependency's version and of the
file's path: changing a macro analyzes again whoever uses it, and editing an ordinary function
does not.

The CLI texts (report, `explain`, errors, hook, MCP and LSP) live in a typed catalog in
`src/i18n/`, one per language; the compiler makes sure every message exists in both.

A mechanical fix (`src/fix.rs`) is a list of range edits (start and end line and column and the
new text) plus the resulting unified diff hunk, computed during the analysis and stored in the
cache with the rest. The LSP sends the edits as a quick fix and `--format agent` sends the diff; a
new rule only has to build the `Fix` (delete a node or insert before one) and it gets
`kerbo fix`, `kerbo_fix` and the LSP's "fix all" for free, which pick the fixes that do not overlap
with `fix::compatible`.

The comments that silence an original tool live in a single registry, `SYNTAXES` in
`src/suppressions.rs`: the comment (`--` or `#`), the marker and whether it applies to the next
line or to the file. Both the anti-cheat and what is ignored when reporting come from there; the
codes are compared with the rule name ignoring case, so a new tool only adds its row.

## Development

The gate (formatting, strict clippy, size ceilings, tests with coverage and `cargo-deny`) is
`scripts/verify-rust.sh` and runs in a container, so all you need is Docker. On Windows, from Git
Bash, run it as `MSYS_NO_PATHCONV=1 bash scripts/verify-rust.sh`: otherwise Git Bash rewrites the
paths passed to `docker run`. `.gitattributes` keeps text files with LF even with `core.autocrlf`
on, so `cargo fmt --check` does not trip over line endings.

## License

[Apache 2.0](LICENSE).
