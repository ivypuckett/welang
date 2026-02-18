# Phase 1: Lexer — Tokenizing welang Source Text

## Goal

Implement a complete lexer (tokenizer) for the welang language. The lexer transforms raw source text into a flat stream of `Token` values, each carrying a `TokenKind` and a `Span` (byte range). This phase does **not** concern itself with parsing, type checking, or code generation — only with recognizing every lexeme the grammar requires.

## Background

welang is a functional language influenced by ML, Lisp, and Forth. Its surface syntax includes:

- **S-expressions** in parentheses for prefix function application
- **Pipe operators** (`|`) for Forth-style postfix composition
- **Curly-brace compound literals** for tuples/objects
- **Square-bracket compound literals** for arrays/maps
- **Type annotation sigils** (`*` for nominal identifiers, `'` for structural aliases)
- **Macro sigil** (`@`) for compile-time value macros
- **Line comments** starting with `#`
- **Two string forms**: double-quoted standard strings and backtick-delimited interpolated strings

## Project Context

The project is a Swift package with this layout:

```
Sources/WeLangLib/
    Lexer.swift      ← you will rewrite most of this file
    Errors.swift     ← you will extend LexError here
Tests/WeLangTests/
    LexerTests.swift ← you will add comprehensive tests here
```

### Current State of `Lexer.swift`

```swift
public struct Token: Equatable {
    public let kind: TokenKind
    public let span: Span
    public init(kind: TokenKind, span: Span) {
        self.kind = kind
        self.span = span
    }
}

public enum TokenKind: Equatable {
    case eof
}

public func lex(_ source: String) throws -> [Token] {
    let length = source.utf8.count
    return [Token(kind: .eof, span: Span(start: length, end: length))]
}
```

### Current State of `LexError`

```swift
public enum LexError: Error, Equatable, CustomStringConvertible {
    case unexpectedCharacter(ch: Character, pos: Int)
    // ...
}
```

### Current State of `Span`

```swift
public struct Span: Equatable, CustomStringConvertible {
    public let start: Int
    public let end: Int
}
```

## Token Catalogue

Below is every `TokenKind` case the lexer must produce. The span of each token covers the byte range `[start, end)` in the UTF-8 source.

### Literals

| Case | Lexeme examples | Notes |
|------|----------------|-------|
| `.integerLiteral(String)` | `0`, `42`, `-1` | Decimal digits, optionally preceded by `-`. Store the raw text so the parser can decide signedness later. |
| `.floatLiteral(String)` | `0.1`, `-3.14` | Digits, a `.`, more digits, optionally preceded by `-`. Must have digits on both sides of the dot. |
| `.stringLiteral(String)` | `"hello"` | Double-quoted. Supports escapes: `\\`, `\"`, `\n`, `\t`, `\r`, `\0`. The stored value is the **unescaped** content (without outer quotes). |
| `.interpolatedStringLiteral(String)` | `` `text {{expr}}` `` | **Temporary stub — replaced in Phase 5.** Backtick-delimited. Store the raw content between backticks as a single string. Validate escapes during scanning: `\{` and `\\` are the only valid escape sequences (stored raw, not processed); any other `\x` throws `LexError.invalidEscape`. |

### Interpolated String Structure Tokens

These five `TokenKind` cases are defined now so the enum is complete, but the lexer **does not emit them until Phase 5** replaces the stub scanner above.

| Case | Notes |
|------|-------|
| `.interpStart` | Opening backtick `` ` `` |
| `.stringSegment(String)` | A literal text segment; escape sequences (`\{` → `{`, `\\` → `\`) are processed; stored value is unescaped |
| `.interpExprOpen` | The `{{` that opens an embedded expression |
| `.interpExprClose` | The `}}` that closes an embedded expression |
| `.interpEnd` | Closing backtick `` ` `` |

Between `.interpExprOpen` and `.interpExprClose`, the lexer emits ordinary tokens for the embedded expression — labels, literals, punctuation, etc.

### Identifiers and Labels

| Case | Lexeme examples | Notes |
|------|----------------|-------|
| `.label(String)` | `foo`, `myVar`, `_private` | Matches `[a-zA-Z_][a-zA-Z0-9_]*`. Contextually, a label followed by `:` becomes the left-hand side of a definition, but the lexer just emits `.label`. |
| `.discard` | `_` | A solitary underscore. If `_` is followed by alphanumerics it is a label; a bare `_` is `.discard`. |

### Punctuation and Delimiters

| Case | Lexeme | Notes |
|------|--------|-------|
| `.colon` | `:` | Definition separator and key-value separator |
| `.comma` | `,` | Element separator in compounds |
| `.dot` | `.` | Tuple/object member access |
| `.pipe` | `\|` | Forth-style postfix combinator |
| `.leftParen` | `(` | S-expression / grouping open |
| `.rightParen` | `)` | S-expression / grouping close |
| `.leftBrace` | `{` | Tuple/object literal open |
| `.rightBrace` | `}` | Tuple/object literal close |
| `.leftBracket` | `[` | Array/map literal open |
| `.rightBracket` | `]` | Array/map literal close |
| `.at` | `@` | Compile-time macro sigil |
| `.star` | `*` | Nominal type (identifier) sigil |
| `.tick` | `'` | Structural type (alias) sigil |
| `.newline` | `\n` | Significant for separating definitions (emit one `.newline` per actual newline; collapse runs of blank lines into one) |
| `.eof` | — | End of input (always the last token) |

### Comments

Comments begin with `#` and extend to the end of the line. **The lexer should skip comments entirely** — do not emit a token for them. Consume all bytes from `#` through (but not including) the next `\n` or end-of-file.

## Lexer Architecture

Implement the lexer as a struct with a cursor-based scanner. Recommended internal design:

```swift
public func lex(_ source: String) throws -> [Token] {
    var lexer = Lexer(source: source)
    return try lexer.scanAll()
}

struct Lexer {
    let source: [UInt8]       // UTF-8 bytes for O(1) indexing
    var pos: Int = 0          // current byte offset

    init(source: String) {
        self.source = Array(source.utf8)
    }

    mutating func scanAll() throws -> [Token] { ... }
    mutating func scanToken() throws -> Token? { ... }
}
```

### Scanning Rules (Priority Order)

1. **Whitespace** (space `0x20`, tab `0x09`, carriage return `0x0D`): skip silently.
2. **Newlines** (`0x0A`): emit `.newline`. Collapse consecutive newlines (possibly separated by whitespace/comments) into a single `.newline` token.
3. **Comments** (`#`): consume to end of line, skip.
4. **Strings** (`"`): scan a standard string literal with escape processing.
5. **Interpolated strings** (`` ` ``): scan to the closing backtick, handling `\{` and `\\` escapes. Store raw content.
6. **Negative numbers** (`-` followed by a digit): scan as negative integer or float literal.
7. **Digits** (`0-9`): scan integer or float literal.
8. **Labels/discard** (`[a-zA-Z_]`): scan identifier. If the result is exactly `_`, emit `.discard`; otherwise emit `.label`.
9. **Single-character punctuation**: `(`, `)`, `{`, `}`, `[`, `]`, `:`, `,`, `.`, `|`, `@`, `*`, `'` — emit the corresponding token.
10. **Anything else**: throw `LexError.unexpectedCharacter(ch:pos:)`.

### Negative Number Disambiguation

A `-` is the start of a negative number **only** when followed immediately by a digit (`0-9`). If `-` appears in any other context, it should be treated as `LexError.unexpectedCharacter` for now (welang has no subtraction operator — math is done through named functions).

### String Escape Processing

For standard strings (`"`), process these escape sequences and store the unescaped result:

| Escape | Meaning |
|--------|---------|
| `\\` | literal backslash |
| `\"` | literal double-quote |
| `\n` | newline (0x0A) |
| `\t` | tab (0x09) |
| `\r` | carriage return (0x0D) |
| `\0` | null (0x00) |

If a backslash is followed by any other character, throw `LexError.invalidEscape(ch:pos:)` (add this case to `LexError`).

If a string is never closed (EOF before closing `"`), throw `LexError.unterminatedString(pos:)` (add this case to `LexError`).

### Interpolated String Scanning

**Phase 1 stub behaviour**: scan everything between the opening and closing backtick and emit a single `.interpolatedStringLiteral(rawContent)` token. The raw content is stored as-is (escapes are not processed). However, validate escapes during scanning: `\{` and `\\` are the only valid escape sequences. If a backslash is followed by any other character, throw `LexError.invalidEscape(ch:pos:)`. This keeps escape validation consistent with standard strings.

If the backtick string is never closed, throw `LexError.unterminatedInterpolatedString(pos:)`.

Phase 5 will replace this stub with a full structured implementation that emits `.interpStart`, interleaved `.stringSegment` / `.interpExprOpen` … `.interpExprClose` groups, and `.interpEnd`.

## Error Cases to Add

Extend `LexError` with these additional cases:

```swift
public enum LexError: Error, Equatable, CustomStringConvertible {
    case unexpectedCharacter(ch: Character, pos: Int)
    case invalidEscape(ch: Character, pos: Int)
    case unterminatedString(pos: Int)
    case unterminatedInterpolatedString(pos: Int)
    case unterminatedInterpolation(pos: Int)  // {{ without matching }} — used starting Phase 5
}
```

Update the `description` computed property to produce clear messages for each case.

## Tests to Write

Place tests in `Tests/WeLangTests/LexerTests.swift`. Preserve the existing tests and add the following. Use the naming convention `test<Category><Scenario>`.

### Comment Tests
- `testLexCommentIsSkipped`: `"# comment\n"` yields `[.newline, .eof]`
- `testLexCommentAtEndOfFile`: `"# comment"` yields `[.eof]` (no trailing newline)
- `testLexCommentAfterToken`: `"foo # comment\n"` yields `[.label("foo"), .newline, .eof]`

### Number Tests
- `testLexUnsignedInteger`: `"42"` → `[.integerLiteral("42"), .eof]`
- `testLexZero`: `"0"` → `[.integerLiteral("0"), .eof]`
- `testLexNegativeInteger`: `"-1"` → `[.integerLiteral("-1"), .eof]`
- `testLexFloatLiteral`: `"3.14"` → `[.floatLiteral("3.14"), .eof]`
- `testLexNegativeFloat`: `"-0.5"` → `[.floatLiteral("-0.5"), .eof]`

### String Tests
- `testLexSimpleString`: `"\"hello\""` → `[.stringLiteral("hello"), .eof]`
- `testLexStringWithEscapes`: `"\"a\\nb\""` → `[.stringLiteral("a\nb"), .eof]`
- `testLexStringWithEscapedQuote`: `"\"she said \\\"hi\\\"\""` → contains the escaped quote
- `testLexUnterminatedString`: `"\"oops"` throws `LexError.unterminatedString`
- `testLexInvalidEscape`: `"\"bad\\x\""` throws `LexError.invalidEscape`
- `testLexInterpolatedString`: `` "`hello {{name}}`" `` → `[.interpolatedStringLiteral("hello {{name}}"), .eof]`
- `testLexUnterminatedInterpolatedString`: `` "`oops" `` throws `LexError.unterminatedInterpolatedString`
- `testLexInvalidEscapeInInterpolatedString`: `` "`bad\\x`" `` throws `LexError.invalidEscape`

### Label and Discard Tests
- `testLexLabel`: `"foo"` → `[.label("foo"), .eof]`
- `testLexLabelWithDigits`: `"x2"` → `[.label("x2"), .eof]`
- `testLexUnderscoredLabel`: `"_private"` → `[.label("_private"), .eof]`
- `testLexDiscard`: `"_"` → `[.discard, .eof]`

### Punctuation Tests
- `testLexAllPunctuation`: lex the string `"(){}[]:,.|@*'"` and verify each token kind in order
- `testLexPipe`: `"|"` → `[.pipe, .eof]`

### Newline Handling Tests
- `testLexNewline`: `"\n"` → `[.newline, .eof]`
- `testLexCollapseBlankLines`: `"\n\n\n"` → `[.newline, .eof]` (collapses to one)
- `testLexNewlineBetweenTokens`: `"a\nb"` → `[.label("a"), .newline, .label("b"), .eof]`

### Compound Expression Tests
- `testLexDefinition`: `"zero: 0"` → `[.label("zero"), .colon, .integerLiteral("0"), .eof]`
- `testLexSExpression`: `"(add 1 2)"` → `[.leftParen, .label("add"), .integerLiteral("1"), .integerLiteral("2"), .rightParen, .eof]`
- `testLexPipedExpression`: `"(1 | 2 | 3)"` → correct token sequence with pipes
- `testLexTupleLiteral`: `"{1, 0.1}"` → correct token sequence
- `testLexMacroApplication`: `"@memoize query"` → `[.at, .label("memoize"), .label("query"), .eof]`
- `testLexTypeAnnotation`: `"*u32"` → `[.star, .label("u32"), .eof]`
- `testLexAliasAnnotation`: `"'u32"` → `[.tick, .label("u32"), .eof]`

### Error Tests
- `testLexUnexpectedCharacter`: `"~"` throws `LexError.unexpectedCharacter`
- Already existing test for `@` should be updated if needed (since `@` is now valid)

### Edge Cases
- `testLexEmptySourceReturnsEof` (existing): verify still passes
- `testLexWhitespaceOnly`: `"   "` → `[.eof]`
- `testLexMultipleDefinitions`: multi-line input with several definitions

## Success Criteria

1. `swift build` compiles without errors.
2. `swift test --filter LexerTests` — all tests pass.
3. `swift test` — full suite passes (existing tests for parser, AST, codegen, and compile should still work since they operate on the `Token`/`Program` types whose shapes haven't changed in a breaking way; adjust if needed).
4. Every `TokenKind` case has at least one test that produces it.
5. Every `LexError` case has at least one test that triggers it.

## Important Notes

- Work in **byte offsets** (`UTF8View`), not `Character` indices, for O(1) random access and correct `Span` values.
- Keep `Token`, `TokenKind`, and `lex()` as `public` so they remain accessible from both the executable and test targets.
- Do **not** modify `Parser.swift`, `AST.swift`, or `Codegen.swift` in this phase (unless a trivial fix is needed to keep them compiling after `TokenKind` changes).
- Run `swift test` before considering this phase complete.
