# How it is built

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
