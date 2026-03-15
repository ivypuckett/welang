# CLAUDE.md

## About welang

welang is a programming language implemented in Rust. The CLI binary is named `we`.

## Syntax

All top-level definitions use `name: body`. The input is always the implicit variable `x`. If the body doesn't reference `x`, the function behaves like a zero-argument function:

```
# function that doesn't use x — behaves like zero-argument
main: 0

# function that uses x — the implicit input variable
double: (multiply [2, x])

# multi-argument operations use tuple syntax [a, b]
factorial:
  {(lessThanOrEqual [x, 1]):
    1,
  _: (multiply [x, (factorial (subtract [x, 1]))])}
```

There is no `define` keyword. Every definition is a function definition.
Bodies are **monadic**: exactly one expression per definition.
Anything else at the top level is a compile-time error.

### Key rules

- All functions use `name: body` syntax. The implicit parameter is always `x`.
- Multi-argument operations use a **tuple**: `[a, b]`.
- Built-in operators (`add subtract multiply divide equal lessThan greaterThan lessThanOrEqual greaterThanOrEqual`) each take a 2-element tuple.
- `{(cond1): v1, (cond2): v2, _: default}` is a conditional expression. Conditions are evaluated left to right; the value of the first truthy arm is returned. The `_` wildcard is required and must be last.
- `{k1: v1, k2: v2}` is a data map literal (keys are plain symbols, not wrapped in `()`).
- `(name: body)` renames the implicit `x` to `name` within `body` (useful for closures).

## Development

```sh
cargo run    # Run the CLI
cargo build  # Build the project
cargo test   # Run the tests
```

## Comments

Use minimal comments — code only, no explanations.

## File size limit

Keep files under **400 lines total** (roughly 200 lines of code and 200 lines of tests). When a file exceeds this limit:

- Break it into its own module with multiple focused files, or
- Refactor the code to be more concise

Do not leave files over 400 lines — split or refactor before committing.

## Before pushing

Always run these and fix any issues before committing/pushing:

```sh
cargo fmt                  # Format code
cargo clippy -- -D warnings  # Lint (all warnings are errors)
```

Also run the cucumber integration tests to make sure no `.we` programs regress:

```sh
cargo test --test cucumber_tests
```
