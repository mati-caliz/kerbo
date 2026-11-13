# Parity with other tools

## Python against ruff

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

## TypeScript and JavaScript against ESLint

In TypeScript and JavaScript, kerbo runs the rules of `@eslint/js` recommended and of
`typescript-eslint` recommended that need no type information, with the ESLint name without the
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

Parity, against ESLint 10.14 and typescript-eslint 8.72: no extra difference by file, line, column
and rule in next.js, react, VS Code, zod and express (42,488 problems in about 30,000 files). 587
problems are missing, all from files tree-sitter does not understand (`in`/`out` variance,
`let as`, `export type *`, escapes in template types) or from `.js` files with type arguments in
calls, which TypeScript reads as comparisons.

## Shell against ShellCheck

`.sh` and `.bash` files and extensionless files with an `sh`, `bash`, `dash` or busybox shebang
are checked against [ShellCheck](https://www.shellcheck.net/), with its code as the rule name,
lowercase: `kerbo explain SC2086`, `--max sc2086=0` or `sc2034 = "off"` in `kerbo.toml`. These are
the 22 rules that show up most, the 64 for what bash adds in an sh script (`sc3001` to `sc3067`), 55 command checks (`tr`, `grep`,
`expr`, `find`, `rm`, `sudo`, `return`, `alias`, `read` and others), 24 on `source`, pipelines and
words (`ls | grep`, `"x$@"`, `$*`, quotes that unquote and others) and 46 on conditions, `case`,
subshells, redirections and assignments, 4 on the variable flow, 43 on comparisons and arithmetic, 35 on `for`, `find -exec`, `exec`, `break`, functions and the shebang, 36 on command names, pipes, redirections and arrays, 19 on the details of `printf`, `getopts`, `time` and declarations 10 on unreachable code and the kind of each variable and 4 more (`sc1003`, `sc1007`, `sc2251`, `sc2316`), which complete ShellCheck's default checks except for what tree-sitter-bash hides; severity is ShellCheck's, `recommended` turns off the style ones
and `minimal` the informational ones too. The [universal rules](reglas.md) do not run on
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
| `sc2003`, `sc2018` to `sc2029`, `sc2060` to `sc2063`, `sc2114` to `sc2313` | ShellCheck's `Checks/Commands.hs`: `tr a-z`, unquoted globs in `find -name`, `grep` and `unset`, `expr`, `rm -rf "$dir/"`, `sudo` with redirections, `return`/`exit` outside 0-255, `export $x`, `alias` with `$1`, `egrep`, `mv` without a destination and the rest |
| `sc1090`, `sc1091`, `sc2007` to `sc2013`, `sc2026`, `sc2027`, `sc2038`, `sc2048`, `sc2070`, `sc2084`, `sc2088`, `sc2091`, `sc2092`, `sc2119`, `sc2120`, `sc2124` to `sc2126`, `sc2140`, `sc2145` | `source` commands that are not followed, `ps \| grep`, `ls \| grep`, `find \| xargs` without `-0`, `for x in $(cat f)`, unquoted `$*`, `"text $@"`, arrays and globs assigned to a variable, `[ -n $x ]`, `"a"$x"b"`, `~` in quotes, `$(cmd)` as a command and functions that read `$1` nobody passes |
| `sc2050`, `sc2076` to `sc2078`, `sc2143`, `sc2144`, `sc2157` to `sc2161`, `sc2193`, `sc2198` to `sc2203`, `sc2208`, `sc2245`, `sc2255`, `sc2268` | conditions that decide nothing or do not do what they seem to: `[ n -le 4 ]`, `[ $a=3 ]`, `[ false ]`, `[ -n "$a " ]`, globs, braces and arrays as operands, `=~ "x.*"`, `[ -n "$(grep x)" ]` and the `x` prefix |
| `sc2001`, `sc2035`, `sc2037`, `sc2044`, `sc2045`, `sc2079`, `sc2094`, `sc2103`, `sc2129`, `sc2141`, `sc2194`, `sc2195`, `sc2209`, `sc2221`, `sc2222`, `sc2231`, `sc2233` to `sc2235`, `sc2269`, `sc2295`, `sc2321`, `sc2327`, `sc2328` | `echo "$x" \| sed`, `rm *`, `A=ls -l`, `for` over `ls` or `find`, reading and writing the same file, `cd` back, several `>>` in a row, `IFS="\n"`, unreachable `case` patterns, `( [ a ] )`, `x=$x`, `${a%$b}`, `a[$((i))]=` and `$(cmd > f)` |
| `sc2017`, `sc2049`, `sc2053` to `sc2058`, `sc2065`, `sc2071` to `sc2074`, `sc2080`, `sc2081`, `sc2099`, `sc2100`, `sc2107` to `sc2110`, `sc2122`, `sc2170`, `sc2171`, `sc2204`, `sc2205`, `sc2212`, `sc2252`, `sc2254`, `sc2257`, `sc2265`, `sc2266`, `sc2283` to `sc2285`, `sc2309`, `sc2322` to `sc2324`, `sc2330`, `sc2331`, `sc2333`, `sc2334` | comparisons that do not compare what they seem to: `[[ $a > 3 ]]`, `[ a && b ]`, `[[ a -o b ]]`, `[ $a = *.txt ]`, `[ $a != x ] \|\| [ $a != y ]`, `test 1 > 2`, `a = 1`, `( -e f )`, `a=(1, 2)`, `i=i+1`, `a+=1`, `08` in arithmetic and extra parentheses |
| `sc2000`, `sc2014`, `sc2032`, `sc2033`, `sc2036`, `sc2041` to `sc2043`, `sc2066`, `sc2067`, `sc2083`, `sc2087`, `sc2093`, `sc2095` to `sc2098`, `sc2101`, `sc2102`, `sc2104` to `sc2106`, `sc2111` to `sc2113`, `sc2118`, `sc2123`, `sc2127`, `sc2147`, `sc2165`, `sc2167`, `sc2187`, `sc2239`, `sc2246`, `sc2258` | the script's flow: a `for` that runs once, unfinished `find -exec`, code after `exec`, `break` outside a loop or in a subshell, `sudo` with a function, `ssh` in a `while read`, `a=1 echo $a`, `[:digit:]`, `PATH=/x`, `function` in sh, the shebang |
| `sc2188` to `sc2192`, `sc2210`, `sc2211`, `sc2215` to `sc2217`, `sc2238`, `sc2247`, `sc2256`, `sc2259` to `sc2264`, `sc2270` to `sc2282`, `sc2286` to `sc2289` | the shape of the command: a lone `> file`, `< f \| cmd`, `foo \| rm`, `a > f \| b`, `a > f > g`, `foo > 2`, `foo > grep`, `*.sh` or `-e` as a command, `$a=1`, `1=a`, `=====`, `foo/`, `ls() { ls; }`, aliases used in their own unit, `$"(ls)"` and `a=(1=b)` |
| `sc2025`, `sc2051`, `sc2175` to `sc2177`, `sc2180`, `sc2182`, `sc2183`, `sc2213`, `sc2214`, `sc2220`, `sc2290`, `sc2291`, `sc2302`, `sc2303`, `sc2318`, `sc2325`, `sc2326`, `sc2332` | the details of a command: `printf` formats with a different number of arguments, the `getopts` `case`, `export a =b`, `declare a=1 b=$a`, `{1..$n}`, `time a \| b` in sh, `echo a    b`, `for i in "${a[@]}"` with `${a[$i]}`, `a \| ! b` and `[ ! -a f ]` |
| `sc2089`, `sc2090`, `sc2128`, `sc2178`, `sc2179`, `sc2218`, `sc2317`, `sc2319`, `sc2320`, `sc2329` | what comes from the flow: functions never called or called before being defined, code after an `exit`, arrays used as text, quotes stored in a variable and a `$?` that is no longer the code of the command meant to be checked |
| `sc2030`, `sc2031`, `sc2153`, `sc2154` | what is assigned inside a subshell and read outside it, and variables nothing in the file assigns (uppercase ones only if a similar one exists) |
| `sc3001` to `sc3067` | in an `sh`, `dash` or busybox script, what only bash has: `[[ ]]`, `local`, arrays, `${x/a/b}`, `echo -n`, `&>`, `source`, `set -o pipefail` and the rest of ShellCheck's `checkBashisms`, with the dash and busybox exceptions |

`# shellcheck disable=` works as in ShellCheck: before the first command, for the whole file;
before a compound command, for the whole block; it accepts ranges (`SC2000-SC2099`) and `all`.

`.shellcheckrc` (or `shellcheckrc`) is read too: the first one found going up from the script, without
leaving the repository. `disable=` sends the findings to the suppressed list with the file name as the
reason, and `shell=` picks the dialect when the script has no directive of its own. In the anti-cheat checks, a
new code in a `disable=` counts as an added exception.

Parity, against ShellCheck 0.11: zero differences by file, line, column and rule on 1,752 scripts
from kubernetes, moby, nvm, rustup, ohmyzsh, acme.sh, bash-it, dokku and others (1,680 findings).
With those repos updated (1,834 scripts, reading the `.shellcheckrc` files): 2,033 identical findings and 87 extra, all
`sc2016` in a newer `nvm.sh` that tree-sitter-bash misreads; without the `# shellcheck` directives,
3,747 identical and none extra. Files tree-sitter-bash cannot parse are
skipped.

## Go against golangci-lint

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
reports), in errcheck 10,754, in `qf1008` and `qf1012` 1,153 and in `sa1019` (use of deprecated
APIs) 592, `qf1001` 136, govet's `stdmethods` 28 and the small staticcheck tail (`st1012`, `qf1011`,
`st1023`, `qf1002`, `s1005`, `sa9003`, `qf1006`, `st1019`, `sa1029`, `sa9004`, `qf1009`, `st1013`,
`s1000`, `st1017`, `st1006`, `s1008`, `s1002` and `s1034`) 204 and in `unused` (code nothing uses) 249, with
none extra: 99 % of what golangci-lint reports by default. It adds 42 more small staticcheck rules
(`s1009`, `s1011`, `s1017`, `s1038`, `sa1006`, `sa2001`, `sa4004`, `sa4016`, `sa6002`, `st1008`,
`qf1007` and more) that match golangci-lint on their test cases and on the 26 reports in the corpus. The rest of govet and staticcheck
stays in golangci-lint.

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

It is squawk 2.71's whole default set except `prefer-repack`, which squawk only runs for
Postgres 19. `recommended` turns off the type preferences (`prefer-*` and `ban-char-field`) and
`minimal` also the timeouts. Parity, against squawk 2.71 with `--assume-in-transaction`: all 9,870
of squawk's reports, none extra, in 1,867 migrations from lobe-chat (drizzle), documenso,
trigger.dev and cal.com (Prisma), conductor (Flyway) and awesome-compose. In Alembic, against squawk
on the SQL Alembic itself generates in offline mode (300 revisions from airflow, superset, mlflow
and redash; the other 349 import the application or query the database and cannot be converted
without running it): 297 identical by rule and count, none with fewer reports, and two with extra
reports on paths offline mode does not take (`is_offline_mode()` and a `try` that breaks offline).

For Django, kerbo reads each migration's `operations` list and builds the SQL `sqlmigrate` would
print for Postgres (Django 6.1): the model state comes from replaying the app's earlier migrations,
with foreign keys into other apps, the through tables of `ManyToManyField`, the `_like` indexes of
`CharField` and names truncated with Django's hash. `RunSQL` contributes its own SQL and
`SeparateDatabaseAndState` only its `database_operations`. Engines built on
django_zero_downtime_migrations (like sentry's) are recognized, along with what their schema editor
adds. Against squawk 2.71 on the real `sqlmigrate` output, by file, rule and count, none extra: all
89 reports of `django.contrib`, 4,296 of saleor, 1,016 of zulip, 1,315 of wagtail and 5,544 of
sentry (`scripts/parity/django/`). What no repo uses (comments, collations, `contrib.postgres`,
moving to `BigAutoField`, M2M fields changing their target) is measured on synthetic cases
(`scripts/parity/django/casos`) against a reference that applies each migration, so what Django
resolves by looking at the database comes out too. An `AlterField` on a foreign key follows
`sqlmigrate`: it does not drop and rebuild the constraint the way a real `migrate` does. Third-party
apps (`auth`, `contenttypes`) are not seen: they are not in the repo.

For Rails, kerbo walks the `change` (or `up`) of the class inheriting from `Migration` with
tree-sitter-ruby and builds the SQL ActiveRecord 8 sends to Postgres, replaying the schema left by
the earlier migrations of the same directory. The reference is squawk 2.71 on the SQL ActiveRecord
really sends when applying the migrations in order on an empty Postgres (`scripts/parity/rails/`):
maybe 849 of 849, chatwoot 1,016 of 1,016, mastodon 1,763 of 1,763, forem 3,556 of 3,556 and
discourse 5,923 of 5,938, none extra. What discourse is missing depends on database data or
application classes; migrations calling the app's models or settings are left out because the
reference does not have them.

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
