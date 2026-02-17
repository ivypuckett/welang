# Phase 2: Parser Foundations — Definitions and Scalar Literals

## Goal

Implement the foundational parser for welang. This phase builds an abstract syntax tree (AST) from the token stream produced by the Phase 1 lexer. You will handle:

- **Definitions** (the only top-level form): `label: value` or `label type: value`
- **Scalar literals**: integer, float, string, interpolated string
- **Labels** (references to other definitions)
- **Discard** (`_`)
- **Implicit input** (`x`)
- **Unit** (empty parentheses `()`)

After this phase, the parser can process programs like:

```we
zero: 0
name: "alice"
pi: 3.14
blank: ()
ignore: _
anInt u32: 23
```

## Background

welang has a uniform top-level structure: every source file is a sequence of **definitions**. There are no bare expressions at the top level. A "function" is just a definition whose value is an s-expression (Phase 3). This design is directly inspired by ML's `let`-binding model, where every construct is a named binding.

Definitions come in two forms:
- **Untyped**: `label: value` — the type is inferred from the value.
- **Typed**: `label type: value` — the type is explicitly specified between the label and the colon.

For example, `anInt u32: 23` declares `anInt` with explicit type `u32` and value `23`. The type annotation is optional and serves as a constraint that the type checker (Phase 7) will validate.

The implicit input variable `x` comes from mathematical convention — every function body can reference its single argument as `x`, mirroring ML's always-monadic (single-argument) function style.

## Project Context

### Files to Modify

```
Sources/WeLangLib/
    AST.swift        ← define AST node types
    Parser.swift     ← implement recursive descent parser
    Errors.swift     ← extend ParseError with new cases
Tests/WeLangTests/
    ASTTests.swift   ← tests for AST node equality
    ParserTests.swift ← comprehensive parser tests
```

### Current Token Types (from Phase 1)

The lexer produces these `TokenKind` values (relevant subset):

```swift
public enum TokenKind: Equatable {
    case integerLiteral(String)
    case floatLiteral(String)
    case stringLiteral(String)
    case interpolatedStringLiteral(String)
    case label(String)
    case discard
    case colon
    case comma
    case dot
    case pipe
    case leftParen, rightParen
    case leftBrace, rightBrace
    case leftBracket, rightBracket
    case at
    case star
    case tick
    case newline
    case eof
}
```

### Current AST (to be replaced)

```swift
public struct Program: Equatable {
    public var items: [Item]
}

public enum Item: Equatable {
    case placeholder(Span)
}
```

## AST Design

Replace the placeholder AST with the following types. All types must be `public` and `Equatable`.

### Program (root node)

```swift
public struct Program: Equatable {
    public var definitions: [Definition]
    public init(definitions: [Definition]) { ... }
}
```

### Definition

A definition binds a label to an expression, with an optional type annotation:

```swift
public struct Definition: Equatable {
    public let label: String
    public let typeAnnotation: Expr?  // optional type between label and colon
    public let value: Expr
    public let span: Span   // covers the full definition from label through value
    public init(label: String, typeAnnotation: Expr?, value: Expr, span: Span) { ... }
}
```

The `typeAnnotation` is the expression between the label and the colon. In this phase, only bare label type names are supported (e.g., `u32` in `anInt u32: 23`). Phase 6 extends this to full type expressions (`*u32`, `'(u32|string)`, etc.).

### Expr (expression node)

Use an indirect enum for the expression type. In this phase, implement only the scalar and atom variants. Later phases will add compound and application forms.

```swift
public indirect enum Expr: Equatable {
    /// Integer literal: `0`, `42`, `-1`
    case integerLiteral(String, Span)

    /// Floating-point literal: `0.1`, `-3.14`
    case floatLiteral(String, Span)

    /// Standard string literal: `"hello"`
    case stringLiteral(String, Span)

    /// Interpolated string literal: `\`hello {{name}}\``
    /// Raw content stored; interpolation parsing is Phase 5.
    case interpolatedStringLiteral(String, Span)

    /// Reference to a name (another definition or built-in): `foo`, `add`
    case name(String, Span)

    /// Implicit input variable: `x`
    /// Parsed as `.name("x", span)` — no special AST node needed.
    /// (Included here for clarity; `x` is just a regular name.)

    /// Discard / wildcard: `_`
    case discard(Span)

    /// Unit value: `()`
    case unit(Span)
}
```

**Design note on `x`**: The implicit input variable `x` is lexed as `.label("x")` and parsed as `.name("x", span)`. There is no special-casing in the parser. Semantic analysis (type inference, Phase 7) will treat `x` as the implicit parameter of the enclosing function. For now, it is just a name reference.

## Parser Architecture

Implement a recursive descent parser. Recommended structure:

```swift
public func parse(_ tokens: [Token]) throws -> Program {
    var parser = Parser(tokens: tokens)
    return try parser.parseProgram()
}

struct Parser {
    let tokens: [Token]
    var pos: Int = 0

    // Peek at the current token without consuming it.
    func peek() -> Token { ... }

    // Consume the current token and advance.
    @discardableResult
    mutating func advance() -> Token { ... }

    // Consume a token of the expected kind, or throw.
    @discardableResult
    mutating func expect(_ kind: TokenKind) throws -> Token { ... }

    // Check if the current token matches a kind.
    func check(_ kind: TokenKind) -> Bool { ... }

    // Skip newline tokens.
    mutating func skipNewlines() { ... }
}
```

**Note on `expect` and `check`**: For `TokenKind` cases with associated values (like `.label(String)`), `check` and `expect` need to match on the case discriminant only, ignoring the associated value. You may want helper methods like:

```swift
func checkLabel() -> String?   // returns the label text if current token is .label
func checkInteger() -> String? // returns the text if current token is .integerLiteral
// etc.
```

### Parsing Rules

#### Program

```
Program = (Newline)* (Definition (Newline)*)* EOF
```

Skip leading newlines, then parse definitions separated by newlines. Trailing newlines are allowed.

#### Definition

```
Definition = Label TypeAnnotation? ":" Expr
TypeAnnotation = Expr    (in this phase, only a bare Label is supported)
```

The current token must be `.label`. Consume it. Then:
- If the next token is `.colon`: no type annotation — consume `:` and parse the value expression.
- If the next token is **not** `.colon`: parse the type annotation as an expression (in this phase, only a label is expected — e.g., `u32`), then expect `.colon`, then parse the value expression.

This handles both `zero: 0` and `anInt u32: 23`. The disambiguation is unambiguous: after a label, `:` means "start the value"; anything else is the type annotation before the co`:`.

**Lookahead strategy**: After the first label, peek at the next token. If it is `.colon`, proceed with no type annotation. If it is `.label` (and the token after *that* is `.colon`), consume the type label. In Phase 6, this will be upgraded to handle `*`, `'`, and compound type expressions.

#### Expr (for this phase)

```
Expr = IntegerLiteral
     | FloatLiteral
     | StringLiteral
     | InterpolatedStringLiteral
     | Label           → Expr.name
     | "_"             → Expr.discard
     | "(" ")"         → Expr.unit
```

The unit case `()` must check for an immediate `.rightParen` after `.leftParen`. If there is content between the parens, that is an s-expression (Phase 3) — for now, throw a `ParseError` for any non-empty parenthesized expression.

### Error Handling

Extend `ParseError` with additional cases:

```swift
public enum ParseError: Error, Equatable, CustomStringConvertible {
    case unexpectedToken(span: Span)
    case expectedColon(span: Span)
    case expectedExpression(span: Span)
    case expectedDefinition(span: Span)
}
```

- `unexpectedToken`: generic fallback for unrecognized syntax
- `expectedColon`: when a definition's `:` is missing
- `expectedExpression`: when an expression is expected but something else appears
- `expectedDefinition`: when a top-level form is not a definition

## Updating Downstream Code

### `Codegen.swift`

The codegen function signature takes a `Program`. Since `Program` now has `definitions` instead of `items`, update the codegen to accept the new structure. For now, the codegen body can be a no-op — just ensure it compiles:

```swift
public func generate(_ program: Program) throws {
    // ... existing LLVM setup ...
    // TODO: walk program.definitions and emit IR
}
```

### `Compile.swift`

No changes needed — it already calls `lex → parse → generate`.

## Tests to Write

### AST Tests (`ASTTests.swift`)

Replace existing placeholder tests with:

- `testDefinitionEquality`: two `Definition` values with same fields are equal
- `testDefinitionInequality`: different labels or values are not equal
- `testDefinitionWithTypeAnnotationEquality`: definitions with same type annotation are equal
- `testDefinitionWithAndWithoutTypeAnnotation`: typed vs untyped definitions are not equal
- `testExprIntegerLiteralEquality`: `.integerLiteral("42", span)` equality
- `testExprFloatLiteralEquality`: `.floatLiteral("3.14", span)` equality
- `testExprStringLiteralEquality`: `.stringLiteral("hi", span)` equality
- `testExprNameEquality`: `.name("foo", span)` equality
- `testExprDiscardEquality`: `.discard(span)` equality
- `testExprUnitEquality`: `.unit(span)` equality
- `testExprDifferentKindsNotEqual`: `.integerLiteral` ≠ `.floatLiteral`

### Parser Tests (`ParserTests.swift`)

Replace existing placeholder tests with:

**Scalar Definitions:**
- `testParseIntegerDefinition`: `"zero: 0"` → one definition with `.integerLiteral("0", _)`
- `testParseNegativeIntegerDefinition`: `"neg: -1"` → `.integerLiteral("-1", _)`
- `testParseFloatDefinition`: `"pi: 3.14"` → `.floatLiteral("3.14", _)`
- `testParseStringDefinition`: `"name: \"alice\""` → `.stringLiteral("alice", _)`
- `testParseInterpolatedStringDefinition`: `` "greeting: `hello {{name}}`" `` → `.interpolatedStringLiteral(...)`

**Typed Definitions:**
- `testParseTypedDefinition`: `"anInt u32: 23"` → definition with `typeAnnotation: .name("u32", _)` and `value: .integerLiteral("23", _)`
- `testParseTypedDefinitionFloat`: `"pi f64: 3.14"` → definition with type annotation `f64`
- `testParseUntypedDefinition`: `"zero: 0"` → definition with `typeAnnotation: nil`

**Names and Discard:**
- `testParseNameReference`: `"alias: other"` → `.name("other", _)`
- `testParseImplicitInput`: `"echo: x"` → `.name("x", _)`
- `testParseDiscard`: `"ignore: _"` → `.discard(_)`

**Unit:**
- `testParseUnit`: `"blank: ()"` → `.unit(_)`

**Multiple Definitions:**
- `testParseMultipleDefinitions`: parse two definitions separated by newline
- `testParseMultipleDefinitionsWithBlankLines`: blank lines between definitions

**Error Cases:**
- `testParseMissingColon`: `"foo 0"` → throws `ParseError.expectedColon`
- `testParseMissingValue`: `"foo:"` followed by newline/eof → throws `ParseError.expectedExpression`
- `testParseBareExpression`: `"42"` at top level → throws `ParseError.expectedDefinition`

**Edge Cases:**
- `testParseEmptySource`: `""` → empty program (0 definitions)
- `testParseOnlyNewlines`: `"\n\n\n"` → empty program
- `testParseTrailingNewline`: `"x: 1\n"` → one definition (trailing newline is fine)

### Codegen Tests (`CodegenTests.swift`)

- Update `testGenerateEmptyProgram` to work with the new `Program(definitions: [])` shape.

### Compile Tests (`CompileTests.swift`)

- `testCompileEmptySource`: should still pass
- `testCompileSimpleDefinition`: `"x: 42"` compiles without error
- `testCompileMultipleDefinitions`: `"a: 1\nb: 2"` compiles without error

## Success Criteria

1. `swift build` compiles without errors.
2. `swift test` — full suite passes.
3. The parser correctly builds an AST for all forms listed above.
4. All error cases produce the correct `ParseError` variant.
5. The `Item` enum and its `.placeholder` case are completely removed — all references replaced with `Definition`.

## Important Notes

- The parser should be **newline-aware**: definitions are separated by newlines (one or more). Newlines within a definition (e.g., between `label:` and the value) are not yet relevant — for now, assume definitions are single-line. Multi-line expressions come in Phase 3.
- The `Expr` enum is `indirect` to allow recursive nesting in future phases.
- Keep all types `public` so tests can access them.
- Run `swift test` before considering this phase complete.
