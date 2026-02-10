# welang

A functional programming language implemented in Rust that compiles to LLVM IR.

## Project Structure

```
src/
├── main.rs      # CLI entry point and top-level compilation pipeline
├── lexer.rs     # Tokenizer: source text → token stream
├── parser.rs    # Parser: token stream → AST
├── ast.rs       # Abstract syntax tree type definitions
├── codegen.rs   # LLVM IR generation via inkwell (LLVM 18 bindings)
└── errors.rs    # Error types (LexError, ParseError, CodegenError, Span)
```

## Compilation Pipeline

```
source text → lexer::lex → parser::parse → codegen::generate → LLVM IR
```

Each phase returns a `Result` with a phase-specific error type. All errors
convert into `CompileError` via `From` impls (derived by `thiserror`).

## Build & Run

```sh
cargo build
cargo run -- <source-file.we>
```

## Dependencies

- **inkwell** (`0.5`, feature `llvm18-0`) — safe Rust bindings to the LLVM C API.
- **thiserror** (`2`) — derive macros for `std::error::Error`.
- **pretty_assertions** (dev, `1`) — readable diffs in test failures.

LLVM 18 must be installed on the system (`llvm-config` must be on `$PATH` or
`LLVM_SYS_180_PREFIX` must be set).

## Testing — IMPORTANT

**Every change to this project must include unit tests.** This is the single
most important development rule for welang.

### Running Tests

```sh
# Run the full test suite
cargo test

# Run tests for a single module
cargo test --lib lexer
cargo test --lib parser
cargo test --lib ast
cargo test --lib codegen
cargo test --lib errors

# Run a single test by name
cargo test lex_empty_source_returns_eof

# Run tests with output shown (useful for debugging)
cargo test -- --nocapture
```

### Testing Guidelines

1. **Every public function must have tests.** If you add or modify a public
   function, write tests that cover the happy path and at least one error
   case.

2. **Test at the unit level first.** Each compiler phase (lexer, parser,
   codegen) should be testable in isolation. Avoid integration tests that
   run the full pipeline unless you are specifically testing end-to-end
   behavior.

3. **Use `pretty_assertions`** for any equality checks on complex types
   (AST nodes, token lists) so failures are easy to diagnose:
   ```rust
   use pretty_assertions::assert_eq;
   ```

4. **Name tests descriptively.** Use the pattern
   `<function_under_test>_<scenario>` — for example
   `lex_string_literal_with_escape`, `parse_missing_semicolon_error`.

5. **Keep tests close to the code.** Tests live in a `#[cfg(test)] mod tests`
   block at the bottom of each module file, not in a separate `tests/`
   directory (integration tests may be added later for end-to-end checks).

6. **Test error cases explicitly.** Ensure that invalid input produces the
   correct error variant and a useful message. Example:
   ```rust
   #[test]
   fn lex_unexpected_character() {
       let err = lex("@").unwrap_err();
       assert!(matches!(err, LexError::UnexpectedCharacter { ch: '@', pos: 0 }));
   }
   ```

7. **Do not skip codegen tests.** The LLVM integration tests (in
   `codegen.rs`) verify that inkwell/LLVM are linked correctly. If they
   fail, the LLVM installation is broken — fix it rather than ignoring it.

8. **Run `cargo test` before every commit.** All tests must pass. Do not
   merge or push code with failing tests.

### What to Test When Adding a Language Feature

When a new language construct is added, expect tests in **all** affected
phases:

| Phase   | What to test                                          |
|---------|-------------------------------------------------------|
| Lexer   | New token types are recognized; edge cases; errors    |
| Parser  | AST nodes are built correctly; precedence; errors     |
| AST     | Equality, cloning, debug output of new node types     |
| Codegen | Correct LLVM IR is emitted; round-trip via JIT if possible |
| main    | End-to-end: source string → successful compilation    |

## Code Style

- Run `cargo clippy` and fix all warnings before committing.
- Run `cargo fmt` to ensure consistent formatting.
- Prefer returning `Result` over panicking. Reserve `unwrap()` for tests.
