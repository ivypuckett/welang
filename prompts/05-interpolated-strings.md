# Phase 5: Interpolated Strings — Template Parsing and Compile-Time Desugaring

## Goal

Implement full parsing of interpolated (backtick-delimited) strings. In Phase 1, the lexer stored the raw content of interpolated strings. In this phase, you will:

1. **Parse interpolation segments** within the raw content, splitting it into static text and `{{expr}}` embedded expressions.
2. **Represent interpolated strings as a desugared AST node** — a sequence of string parts and expression parts.
3. Mark interpolated strings for **compile-time evaluation** — the interpolation is resolved at compile time (no runtime string formatting cost).

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
- All other characters (including newlines) are literal.
- Interpolated strings are desugared to concatenation at **compile time**. There is zero runtime cost compared to a plain string.

### Why Compile-Time?

Interpolated strings are syntactic sugar. During compilation, `` `hello {{name}}` `` is transformed into the equivalent of `(concat "hello " (toString name))`. This happens before code generation, so the backend only ever sees plain string operations. This is the same approach as Rust's `format!` macro or Zig's `comptime` string formatting — the interpolation template is fully resolved during compilation.

## Project Context

### Files to Modify

```
Sources/WeLangLib/
    AST.swift        ← add interpolation segment types, update Expr
    Parser.swift     ← add interpolation parsing pass
    Errors.swift     ← add interpolation-specific errors
Tests/WeLangTests/
    ParserTests.swift ← interpolation tests
    ASTTests.swift    ← segment equality tests
```

### Current State

The lexer (Phase 1) produces `.interpolatedStringLiteral(String)` tokens where the `String` is the **raw content** between backticks, with `\{` and `\\` escape sequences still present as-is. For example:

```
`hello {{name}}`
```

The lexer emits: `.interpolatedStringLiteral("hello {{name}}")`

And:

```
`literal \{{not interpolated}}`
```

The lexer emits: `.interpolatedStringLiteral("literal \\{{not interpolated}}")`

Note: the lexer handles `\{` and `\\` by passing them through as literal `\{` and `\\` in the raw string. The parser must now interpret these.

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

## Parsing Logic

### Interpolation Parsing

When the parser encounters an `.interpolatedStringLiteral(rawContent)` token, it must parse the raw content into segments. Implement this as a separate function:

```swift
func parseInterpolatedString(raw: String, baseSpan: Span) throws -> [InterpolationSegment]
```

#### Algorithm

Walk through the raw string character by character:

1. **Accumulate text** into a buffer until you hit `\`, `{`, or end of string.

2. **Escape handling** (`\`):
   - `\{` → append literal `{` to the text buffer
   - `\\` → append literal `\` to the text buffer
   - Any other `\x` → throw an error (the lexer should have caught this, but be defensive)

3. **Interpolation start** (`{{`):
   - Flush the current text buffer as a `.text` segment (if non-empty)
   - Scan forward to find the matching `}}`
   - Extract the expression source between `{{` and `}}`
   - **Lex and parse** the extracted expression source as a welang expression
   - Append the result as an `.expression` segment
   - Continue after `}}`

4. **Single `{`** (not `{{`):
   - This is a literal `{` character — append to text buffer. (Only `{{` triggers interpolation.)

5. **End of string**: flush any remaining text buffer as a `.text` segment.

#### Nested Parsing

The expression inside `{{...}}` is parsed by calling the existing welang lexer and expression parser on the extracted substring. This means interpolated expressions support the full welang expression syntax:

```we
complex: `result is {{(add 1 2)}}`
access: `name is {{x.name}}`
nested: `value is {{data[0]}}`
```

To parse the inner expression:
1. Call `lex()` on the extracted substring
2. Create a sub-parser from the resulting tokens
3. Call `parseExpr()` to get the expression AST
4. Verify the sub-parser has consumed all tokens (except `.eof`)

#### Finding Matching `}}`

Scan from the opening `{{` and count brace nesting depth:
- Each `{` increments depth
- Each `}` decrements depth
- When you encounter `}}` and depth returns to 0, that is the matching close

This handles nested braces in expressions like `{{map.{ key: value }}}` — but in practice, most interpolations will be simple names or function calls.

### Error Cases

Add to relevant error types:

```swift
// In ParseError or a new InterpolationError:
case unterminatedInterpolation(pos: Int)    // {{ without matching }}
case emptyInterpolation(pos: Int)           // {{}} with nothing inside
case interpolationParseError(String, pos: Int) // expression inside {{}} failed to parse
```

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

- **The lexer does NOT change in this phase**. It still produces `.interpolatedStringLiteral(rawContent)` tokens. The parser is what transforms the raw content into parsed segments.
- **Nested parsing**: you are calling `lex()` and a sub-parser inside the parser. Make sure errors from the nested parse are properly wrapped or propagated.
- **Span tracking**: spans for interpolation segments are offsets within the interpolated string. You may need to adjust them relative to the token's base span.
- Keep all types `public` and `Equatable`.
- Run `swift test` before considering this phase complete.
