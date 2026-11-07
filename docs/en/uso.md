# Usage

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
(`.env`, `.npmrc`) are only checked for secrets. Python walks like ruff: hidden files too, except
its default excludes (`.venv`, `venv`, `dist`, `site-packages` and the like). It exits with `0` when there are no problems, `1`
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

## Fixing everything at once

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

## Adopting it in an existing repo

`kerbo baseline` saves the current problems in `kerbo-baseline.json` (commit it). From then on,
`kerbo check` reports only new problems and the ones that **got worse**: if a function in the
baseline goes from complexity 14 to 16, it shows up again. Each problem is identified by rule,
file and function, not by line number, so moving code around does not make it "new".
`--no-baseline` shows everything.

## Anti-cheat

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
