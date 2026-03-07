# CLAUDE.md

## About welang

welang is a programming language implemented in Rust. The CLI binary is named `we`.

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
