# CLAUDE.md

## About welang

welang is a programming language implemented in Rust. The CLI binary is named `we`.

## Development

```sh
cargo run    # Run the CLI
cargo build  # Build the project
cargo test   # Run the tests
```

## Language semantics

### Monadic currying

Function calls are exclusively monadic: every application takes exactly
one argument at a time. Multi-argument functions are called in curried
form.

```lisp
; Define a two-argument function normally
(define (add x y) ((+ x) y))

; Call it with curried syntax — one argument per application
((add 1) 2)          ; => 3

; Built-in operators follow the same rule
((+ 1) 2)            ; => 3
((* 4) ((+ 1) 2))    ; => 12

; Passing multiple args at once is a compile error
(add 1 2)            ; ERROR: use ((add 1) 2)
(+ 1 2)              ; ERROR: use ((+ 1) 2)
```

## Before pushing

Always run these and fix any issues before committing/pushing:

```sh
cargo fmt                  # Format code
cargo clippy -- -D warnings  # Lint (all warnings are errors)
```
