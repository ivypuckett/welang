# Phase 5: Interpolated Strings — Template Parsing and Compile-Time Desugaring

## Goal

Implement full support for interpolated (backtick-delimited) strings. In Phase 1, the lexer stored raw backtick content as a single stub token. In this phase, you will:

1. **Rewrite the interpolated string scanner in `Lexer.swift`** to emit a structured token sequence: `.interpStart`, interleaved `.stringSegment` and expression tokens grouped by `.interpExprOpen`/`.interpExprClose`, and `.interpEnd`.
2. **Update the parser** to consume the structured token sequence and build an `InterpolationSegment` list.
3. **Represent interpolated strings as an AST node** — a sequence of string and expression parts.
4. Mark interpolated strings for **compile-time evaluation** — the interpolation is resolved at compile time (no runtime string formatting cost).

After this phase:

```we
greeting: `hello {{name}}, you are {{age}} years old`
# Desugars to: concat("hello ", toString(name), ", you are ", toString(age), " years old")

escaped: `literal \{{ not interpolated }}`
# The \{{ produces a literal "{{" — no interpolation

multiline: `first line
second line
and {{value}} here`
```

## Background

welang has two string forms:

1. **Standard strings** (`"..."`): already fully handled — no interpolation, just escape sequences.
2. **Interpolated strings** (`` `...` ``): backtick-delimited, supporting `{{expr}}` interpolation and multi-line content.

### Interpolation Rules

- `{{expr}}` — the expression between double braces is evaluated and converted to a string. The expression follows normal welang expression syntax.
- `\{` — escape sequence producing a literal `{` character (prevents interpolation).
- `\\` — escape sequence producing a literal `\` character.
- `` \` `` — escape sequence producing a literal backtick character (allows backticks inside the string).
- All other characters (including newlines) are literal.
- Interpolated strings are desugared to concatenation at **compile time**. There is zero runtime cost compared to a plain string.

### Why Compile-Time?

Interpolated strings are syntactic sugar. During compilation, `` `hello {{name}}` `` is transformed into the equivalent of `(concat "hello " (toString name))`. This happens before code generation, so the backend only ever sees plain string operations. This is the same approach as Rust's `format!` macro or Zig's `comptime` string formatting — the interpolation template is fully resolved during compilation.

## Project Context

### Files to Modify

```
Sources/WeLangLib/
    Lexer.swift      ← replace stub scanner with structured token emission; remove .interpolatedStringLiteral
    AST.swift        ← add InterpolationSegment type, update Expr
    Parser.swift     ← consume structured tokens instead of re-lexing raw content
    Errors.swift     ← add interpolation-specific parse errors (LexError.unterminatedInterpolation already defined in Phase 1)
Tests/WeLangTests/
    LexerTests.swift  ← structured token emission tests
    ParserTests.swift ← interpolation AST tests
    ASTTests.swift    ← segment equality tests
```

### Current State

The lexer (Phase 1) currently produces a single stub token for any backtick string:

```
`hello {{name}}`  →  .interpolatedStringLiteral("hello {{name}}")
```

After the Phase 5 lexer rewrite, the same input will produce:

```
`hello {{name}}`
→  .interpStart
   .stringSegment("hello ")
   .interpExprOpen
   .label("name")
   .interpExprClose
   .interpEnd
```

The stub token `.interpolatedStringLiteral` is removed from `TokenKind` at the end of this phase. Any phase 2–4 code that references it will need to be updated (in practice, earlier phases pass interpolated strings through without inspecting the raw content, so changes should be minimal).

## AST Additions

### Interpolation Segment

```swift
/// A segment of an interpolated string.
public enum InterpolationSegment: Equatable {
    /// A literal text segment: static string content.
    case text(String, Span)

    /// An interpolated expression segment: `{{expr}}`.
    case expression(Expr, Span)
}
```

### Updated Expr Case

Replace the existing `.interpolatedStringLiteral(String, Span)` case:

```swift
public indirect enum Expr: Equatable {
    // ... other cases ...

    /// Interpolated string, now fully parsed into segments.
    /// `\`hello {{name}}\`` → .interpolatedString(segments: [.text("hello "), .expression(.name("name"))])
    case interpolatedString(segments: [InterpolationSegment], Span)

    // Remove or replace: case interpolatedStringLiteral(String, Span)
}
```

**Migration**: Any existing code that references `.interpolatedStringLiteral` should be updated to use `.interpolatedString`. The parser transforms the raw lexer token into parsed segments.

## Lexer Rewrite

Replace the Phase 1 stub in `Lexer.swift`. When the scanner hits a backtick (`` ` ``):

1. Emit `.interpStart`.
2. Start an empty text buffer. Walk bytes:
   - `` \` `` → append `` ` `` to buffer (escape processed).
   - `\` + `{` → append `{` to buffer (escape processed).
   - `\` + `\` → append `\` to buffer (escape processed).
   - `\` + anything else → throw `LexError.invalidEscape(ch:pos:)`.
   - `{` + `{` → flush buffer as `.stringSegment(buf)` if non-empty, emit `.interpExprOpen`, enter **expression mode**.
   - `` ` `` (unescaped) → flush buffer as `.stringSegment(buf)` if non-empty, emit `.interpEnd`, done.
   - EOF → throw `LexError.unterminatedInterpolatedString(pos:)`.
   - Any other byte → append to buffer.

**Expression mode** — emit normal tokens until the closing `}}`:

- Track brace depth, starting at 0.
- `{` → emit `.leftBrace`, depth++.
- `}` at depth > 0 → emit `.rightBrace`, depth--.
- `}` at depth == 0 → peek at the next byte:
  - Next byte is `}` → emit `.interpExprClose`, advance past both `}}`, return to text mode.
  - Next byte is not `}` → throw `LexError.unexpectedCharacter(ch: "}", pos:)`.
- EOF → throw `LexError.unterminatedInterpolation(pos:)`.
- All other bytes → scan normally (whitespace, labels, literals, punctuation).

This handles nested braces correctly: `{{ {k: 1} }}` has depth 1 inside the tuple literal; only the outer `}}` has depth 0.

After implementing this, **remove `.interpolatedStringLiteral` from `TokenKind`**.

## Parser Integration

The parser no longer re-lexes raw content. When it encounters `.interpStart`, collect segments until `.interpEnd`:

```swift
// Pseudocode
case .interpStart:
    var segments: [InterpolationSegment] = []
    while !at(.interpEnd) {
        if at(.stringSegment):
            segments.append(.text(content, span))
            advance()
        else if at(.interpExprOpen):
            advance()
            let expr = try parseExpr()
            expect(.interpExprClose)
            segments.append(.expression(expr, span))
        else:
            throw ParseError.unexpected(...)
    }
    expect(.interpEnd)
    return .interpolatedString(segments: segments, span)
```

This is significantly simpler than the old approach — no sub-lexing, no offset arithmetic. Expressions inside `{{...}}` are just normal tokens already in the stream, so a standard `parseExpr()` call handles them.

### Error Cases

`LexError.unterminatedInterpolation(pos:)` is already defined in Phase 1 — no changes to `LexError` needed.

Add to `ParseError`:

```swift
case emptyInterpolation(pos: Int)   // {{}} with nothing inside — caught by the parser
```

`interpolationParseError` is no longer needed: since the lexer emits real tokens for the embedded expression, any parse failure inside `{{...}}` is a normal `ParseError` from the expression parser with a correct source position.

## Desugaring Strategy

At the AST level, an interpolated string is represented as `.interpolatedString(segments:)`. The actual desugaring to `concat(toString(a), b, toString(c), ...)` happens in a later compilation phase (semantic analysis or codegen). For now, the parser just produces the segment list.

In a future phase, the desugaring would transform:

```
.interpolatedString(segments: [
    .text("hello "),
    .expression(.name("name")),
    .text(", you are "),
    .expression(.name("age")),
    .text(" years old")
])
```

Into:

```
.apply(
    function: .name("concat"),
    arguments: [
        .stringLiteral("hello "),
        .apply(function: .name("toString"), arguments: [.name("name")]),
        .stringLiteral(", you are "),
        .apply(function: .name("toString"), arguments: [.name("age")]),
        .stringLiteral(" years old")
    ]
)
```

But that transformation is **not** part of this phase — just store the segments.

## Tests to Write

### Lexer Tests

Place in `LexerTests.swift`.

**Structured token emission:**
- `testLexInterpStringNoInterpolation`: `` "`just text`" `` → `[.interpStart, .stringSegment("just text"), .interpEnd, .eof]`
- `testLexInterpStringOneExpr`: `` "`hello {{name}}`" `` → `[.interpStart, .stringSegment("hello "), .interpExprOpen, .label("name"), .interpExprClose, .interpEnd, .eof]`
- `testLexInterpStringOnlyExpr`: `` "`{{x}}`" `` → `[.interpStart, .interpExprOpen, .label("x"), .interpExprClose, .interpEnd, .eof]`
- `testLexInterpStringMultipleExprs`: `` "`{{a}} and {{b}}`" `` → correct interleaved segments
- `testLexInterpStringEscapedBrace`: `` "`\\{not interpolated}`" `` → `[.interpStart, .stringSegment("{not interpolated}"), .interpEnd, .eof]` (single `\{` produces `{`; subsequent characters are literal)
- `testLexInterpStringEscapedBackslash`: `` "`a \\\\  b`" `` → `.stringSegment` containing literal `\`
- `testLexInterpStringEscapedBacktick`: `` "`a \\` b`" `` → `[.interpStart, .stringSegment("a ` b"), .interpEnd, .eof]`
- `testLexInterpStringNestedBraces`: `` "`{{ {k: 1} }}`" `` → `.interpExprOpen`, `.leftBrace`, `.label("k")`, `.colon`, `.integerLiteral("1")`, `.rightBrace`, `.interpExprClose`
- `testLexInterpStringMultiline`: backtick string with a literal newline in the text → the newline appears in the `.stringSegment` content; no `.newline` token is emitted mid-string
- `testLexUnterminatedInterpolation`: `` "`hello {{name`" `` throws `LexError.unterminatedInterpolation`

### AST Tests

- `testInterpolationSegmentTextEquality`: `.text("hi", _) == .text("hi", _)`
- `testInterpolationSegmentExprEquality`: `.expression(.name("x"), _) == .expression(.name("x"), _)`
- `testInterpolationSegmentInequality`: `.text("a") ≠ .text("b")`
- `testInterpolatedStringEquality`: two `.interpolatedString` with same segments

### Parser Tests

**Basic interpolation:**
- `testParseInterpolatedStringNoInterpolation`: `` "s: `just text`" `` → `.interpolatedString(segments: [.text("just text")])`
- `testParseInterpolatedStringOneExpr`: `` "s: `hello {{name}}`" `` → two segments: text + expression
- `testParseInterpolatedStringMultipleExprs`: `` "s: `{{a}} and {{b}}`" `` → five segments: text("") or omitted, expr(a), text(" and "), expr(b), text("") or omitted
- `testParseInterpolatedStringOnlyExpr`: `` "s: `{{x}}`" `` → single expression segment

**Escape sequences:**
- `testParseInterpolatedStringEscapedBrace`: `` "s: `\\{{not interpolated}}`" `` → text with literal `{`
- `testParseInterpolatedStringEscapedBackslash`: `` "s: `a \\\\ b`" `` → text with literal `\`

**Complex expressions in interpolation:**
- `testParseInterpolatedStringWithApply`: `` "s: `result: {{(add 1 2)}}`" `` → expression segment contains `.apply`
- `testParseInterpolatedStringWithAccess`: `` "s: `name: {{x.name}}`" `` → expression segment contains `.dotAccess`

**Multi-line:**
- `testParseInterpolatedStringMultiline`: backtick string spanning multiple lines preserves newlines in text segments

**Error cases:**
- `testParseUnterminatedInterpolation`: `` "s: `hello {{name`" `` → throws unterminated interpolation error
- `testParseEmptyInterpolation`: `` "s: `hello {{}}`" `` → throws empty interpolation error

**Edge cases:**
- `testParseInterpolatedStringSingleBrace`: `` "s: `a { b`" `` → text segment containing `{` (single brace is literal)
- `testParseInterpolatedStringAdjacentExprs`: `` "s: `{{a}}{{b}}`" `` → two expression segments back-to-back

### Compile Tests

- `testCompileInterpolatedString`: `` "s: `hello {{name}}`" `` compiles without error

## Success Criteria

1. `swift build` compiles without errors.
2. `swift test` — full suite passes.
3. Interpolated strings are parsed into `.interpolatedString(segments:)` with correct text and expression segments.
4. Escape sequences (`\{`, `\\`) are handled correctly.
5. Expressions inside `{{...}}` are fully parsed using the existing expression parser.
6. Error cases (unterminated, empty) produce correct errors.
7. The old `.interpolatedStringLiteral` case is removed from the AST, replaced by `.interpolatedString`.

## Important Notes

- **The lexer changes significantly in this phase**. The stub `.interpolatedStringLiteral` token is replaced by the five-token structured sequence. Update any phase 2–4 code that referenced the old token.
- **No sub-lexing**: embedded expression tokens are emitted directly into the main token stream, so all `Span` values are correct relative to the original source with no offset adjustment needed.
- **Span tracking**: `.interpExprOpen` and `.interpExprClose` spans cover the `{{` and `}}` byte ranges respectively; `.stringSegment` spans cover the raw byte range in the source (the stored string may be shorter after escape processing).
- Keep all types `public` and `Equatable`.
- Run `swift test` before considering this phase complete.
