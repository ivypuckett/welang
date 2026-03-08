# CLAUDE.md

## About welang

welang is a programming language implemented in Rust. The CLI binary is named `we`.

## Syntax

All top-level definitions use `name: (params) body`:

```
; no-argument function
main: () 0

; function with parameters
add: (a b) (+ a b)

; single-expression body (bodies are monadic — exactly one expression)
factorial: (n)
  (if (<= n 1)
    1
    (* n (factorial (- n 1))))
```

There is no `define` keyword. Every definition is a function definition.
Bodies are **monadic**: exactly one expression follows the parameter list.
Anything else at the top level is a compile-time error.

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
