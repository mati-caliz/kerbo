# Rules

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

Those are the thresholds of the `strict` preset; the other two presets are in [Configuration](configuracion.md).

## Rust

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
