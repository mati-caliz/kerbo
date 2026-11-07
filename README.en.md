# kerbo

[Español](README.md) · English

**One linter for the whole repo.** kerbo checks code quality in Java, TypeScript/JavaScript,
Python, Go and Rust, plus shell scripts, Postgres migrations, Dockerfiles and docker compose
files. It is a single binary, it takes seconds and it gives the same results as the tool you
already use for each of them.

```console
$ kerbo check --lang en
src/billing/InvoiceService.java:88:5: cyclomatic-complexity The function `settle` has cyclomatic complexity 11 (maximum 8)
web/src/orders/use-cart.ts:42:1: cognitive-complexity The function `applyPromotions` has cognitive complexity 19 (maximum 15)
kerbo: 5810 files, 2 problems, 2250 ms
```

## Why

- **It replaces several tools** without changing their criteria. Each part is compared against
  the original on large real repos (django, next.js, kubernetes, terraform…) until it reports the
  same warnings at the same file, line and column:

  | Checks | Same as |
  |---|---|
  | Python | ruff (`E4`, `E7`, `E9`, `F`) |
  | TypeScript and JavaScript | ESLint and typescript-eslint recommended |
  | Go | golangci-lint (errcheck, ineffassign, staticcheck, unused) |
  | Java | Checkstyle (`sun_checks`), PMD and several IntelliJ inspections |
  | Rust | clippy (length, parameters, complexity and nesting) |
  | Shell | ShellCheck |
  | Postgres migrations (SQL, Prisma, drizzle, Flyway, Alembic) | squawk |
  | Dockerfile | hadolint |
  | Committed secrets | gitleaks |

- **The same metrics in every language**: file and function length, cyclomatic and cognitive
  complexity, parameters, nesting, magic numbers and duplicate code.
- **Nothing is loosened silently.** Every exception goes in `kerbo.toml` with its reason, and with
  `--diff` it flags what a change adds to dodge a problem instead of fixing it: an
  `eslint-disable`, a skipped or deleted test, an `any`.
- **Built for AI agents**: a Claude Code hook that checks every file the agent edits, an MCP
  server and a compact output that spends few tokens.
- **Fast**: it analyzes in parallel and keeps a content cache, so the second run only looks at
  what changed.

## Installation

```sh
curl -fsSL https://kerbo.matiascaliz.com.ar/install.sh | sh      # Linux, macOS and Git Bash on Windows
irm https://kerbo.matiascaliz.com.ar/install.ps1 | iex           # PowerShell on Windows
brew install mati-caliz/tap/kerbo                                # Homebrew
cargo install kerbo --locked                                     # with Rust
```

The installer downloads the binary from the [releases](https://github.com/mati-caliz/kerbo/releases),
verifies its sha256 and puts it in `~/.local/bin` (on Windows, in `%LOCALAPPDATA%\Programs\kerbo`).

## Usage

```sh
kerbo check                  # check the current directory
kerbo check --diff main      # only the lines changed since main
kerbo fix                    # apply the mechanical fixes
kerbo baseline               # accept what is there: from now on only new problems are reported
kerbo explain <rule>         # why a rule exists and how to fix it
```

It exits with `0` when there are no problems, `1` when there are and `2` when something could not
be read. Without any configuration it uses the strictest preset; to loosen it, add a `kerbo.toml`
at the repo root:

```toml
preset = "recommended"            # strict (default), recommended or minimal
lang = "en"                       # messages in English

[[exceptions]]
paths = ["src/legacy/**"]
rule = "cognitive-complexity"
reason = "being rewritten next quarter"
```

In GitHub Actions, problems show up as annotations on the pull request:

```yaml
- uses: mati-caliz/kerbo@v0.2.0
  with:
    args: --diff origin/${{ github.base_ref }}
```

With Claude Code, add the hook to `.claude/settings.json` and kerbo hands the agent whatever it
finds on every edit, so it fixes it in the same turn:

```json
{ "hooks": { "PostToolUse": [{ "matcher": "Edit|MultiEdit|Write", "hooks": [{ "type": "command", "command": "kerbo hook" }] }] } }
```

## Documentation

- [Usage](docs/en/uso.md): every option, the cache, `kerbo fix`, the baseline and the anti-cheat.
- [Configuration](docs/en/configuracion.md): `kerbo.toml`, the presets and GitHub Actions.
- [AI agents and editors](docs/en/agentes-y-editores.md): the hook, the daemon, MCP and LSP.
- [Rules](docs/en/reglas.md): the shared metrics and their thresholds per language.
- [Java](docs/en/java.md): Checkstyle, the IntelliJ inspections and how it reads libraries.
- [Security](docs/en/seguridad.md): injections, secrets and vulnerable dependencies.
- [Parity](docs/en/paridad.md): every tool it replaces, rule by rule and measured against which
  repos.
- [How it is built](docs/en/arquitectura.md) and how to run the development gate.
- [Roadmap](ROADMAP.md) (in Spanish).

## License

[Apache 2.0](LICENSE).
