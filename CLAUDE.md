# CLAUDE.md

## About welang

welang is a programming language implemented in Rust. The CLI binary is named `we`.

## Syntax

All top-level definitions use `name: body`. The input is always the implicit variable `x`. If the body doesn't reference `x`, the function behaves like a zero-argument function:

```
; function that doesn't use x — behaves like zero-argument
main: 0

; function that uses x — the implicit input variable
double: (* [2, x])

; multi-argument operations use tuple syntax [a, b]
factorial:
  (if [(<= [x, 1]),
    1,
    (* [x, (factorial (- [x, 1]))])])
```

There is no `define` keyword. Every definition is a function definition.
Bodies are **monadic**: exactly one expression per definition.
Anything else at the top level is a compile-time error.

### Key rules

- All functions use `name: body` syntax. The implicit parameter is always `x`.
- Multi-argument operations use a **tuple**: `[a, b]`.
- Built-in operators (`+ - * / = < > <= >=`) each take a 2-element tuple.
- `if` takes a 2- or 3-element tuple: `(if [cond, then])` or `(if [cond, then, else])`.
- `(name: body)` renames the implicit `x` to `name` within `body` (useful for closures).

## Development

```sh
cargo run    # Run the CLI
cargo build  # Build the project
cargo test   # Run the tests
```

## Before pushing

Always run these and fix any issues before committing/pushing:

```sh
cargo fmt                  # Format code
cargo clippy -- -D warnings  # Lint (all warnings are errors)
```

Also compile every file in `tests/` to make sure none of them regress:

```sh
cargo build
for f in tests/*.we; do echo "Compiling $f ..."; ./target/debug/we "$f"; done
```
