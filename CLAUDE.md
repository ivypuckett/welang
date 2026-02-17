# welang

A functional programming language implemented in Swift that compiles to LLVM IR.

## Project Structure

```
Sources/
├── CLLLVM/
│   └── include/
│       ├── module.modulemap   # C module map for LLVM-C API
│       └── shim.h             # Umbrella header importing LLVM-C headers
├── WeLangLib/
│   ├── Compile.swift          # Top-level compilation pipeline
│   ├── Lexer.swift            # Tokenizer: source text → token stream
│   ├── Parser.swift           # Parser: token stream → AST
│   ├── AST.swift              # Abstract syntax tree type definitions
│   ├── Codegen.swift          # LLVM IR generation via LLVM-C API
│   └── Errors.swift           # Error types (LexError, ParseError, CodegenError, Span)
└── WeLang/
    └── main.swift             # CLI entry point

Tests/
└── WeLangTests/
    ├── ErrorsTests.swift      # Error type and Span tests
    ├── LexerTests.swift       # Tokenizer tests
    ├── ParserTests.swift      # Parser tests
    ├── ASTTests.swift         # AST type tests
    ├── CodegenTests.swift     # LLVM code generation tests
    └── CompileTests.swift     # End-to-end compilation tests
```

## Compilation Pipeline

```
source text → lex() → parse() → generate() → LLVM IR
```

Each phase throws a phase-specific error type. The top-level `compile()`
function in `Compile.swift` orchestrates the pipeline and propagates errors
as `CompileError`.

## Build & Run

```sh
swift build
swift run welang <source-file.we>
```

## Dependencies

- **CLLLVM** (system library) — C module map bridging to the LLVM-C API.
- **LLVM 18** must be installed on the system with development headers.

### Installing LLVM 18 (Ubuntu)

```sh
sudo apt install llvm-18-dev
```

Ensure `llvm-config-18` (or `llvm-config`) is on `$PATH`, or set the
`PKG_CONFIG_PATH` to include the LLVM 18 pkgconfig directory.

## Testing — IMPORTANT

**Every change to this project must include unit tests.** This is the single
most important development rule for welang.

### Running Tests

```sh
# Run the full test suite
swift test

# Run tests for a single test class
swift test --filter ErrorsTests
swift test --filter LexerTests
swift test --filter ParserTests
swift test --filter ASTTests
swift test --filter CodegenTests
swift test --filter CompileTests

# Run a single test by name
swift test --filter testLexEmptySourceReturnsEof

# Run tests with verbose output
swift test --verbose
```

### Testing Guidelines

1. **Every public function must have tests.** If you add or modify a public
   function, write tests that cover the happy path and at least one error
   case.

2. **Test at the unit level first.** Each compiler phase (lexer, parser,
   codegen) should be testable in isolation. Avoid integration tests that
   run the full pipeline unless you are specifically testing end-to-end
   behavior.

3. **Name tests descriptively.** Use the pattern
   `test<FunctionUnderTest><Scenario>` — for example
   `testLexStringLiteralWithEscape`, `testParseMissingSemicolonError`.

4. **Keep tests organized by module.** Each source file has a corresponding
   test file in `Tests/WeLangTests/`.

5. **Test error cases explicitly.** Ensure that invalid input produces the
   correct error variant and a useful message. Example:
   ```swift
   func testLexUnexpectedCharacter() throws {
       XCTAssertThrowsError(try lex("@")) { error in
           guard case LexError.unexpectedCharacter(ch: "@", pos: 0) = error else {
               XCTFail("Unexpected error: \(error)")
               return
           }
       }
   }
   ```

6. **Do not skip codegen tests.** The LLVM integration tests (in
   `CodegenTests.swift`) verify that LLVM is linked correctly. If they
   fail, the LLVM installation is broken — fix it rather than ignoring it.

7. **Run `swift test` before every commit.** All tests must pass. Do not
   merge or push code with failing tests.

### What to Test When Adding a Language Feature

When a new language construct is added, expect tests in **all** affected
phases:

| Phase   | What to test                                          |
|---------|-------------------------------------------------------|
| Lexer   | New token types are recognized; edge cases; errors    |
| Parser  | AST nodes are built correctly; precedence; errors     |
| AST     | Equality of new node types                            |
| Codegen | Correct LLVM IR is emitted; round-trip via JIT if possible |
| Compile | End-to-end: source string → successful compilation    |

## Code Style

- Follow Swift API Design Guidelines.
- Use `throws` for fallible functions instead of force-unwrapping. Reserve
  `try!` and force-unwraps for tests only.
- Mark types and functions `public` in `WeLangLib` so they are accessible
  from both the executable target and the test target.
