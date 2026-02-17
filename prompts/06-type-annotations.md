# Phase 6: Type Annotations — Aliases, Identifiers, and Type Declarations

## Goal

Implement the type annotation syntax for welang, adding:

1. **Alias types** (`'`): structural types (ML-style type aliases) — any value matching the structure satisfies the type
2. **Identifier types** (`*`): nominal types (ML-style datatypes) — only values explicitly tagged with the type name satisfy it
3. **Function types**: `*(inType|outType)` or `'(inType|outType)`
4. **Compound types**: type annotations for tuples/objects `*{k:T}` and arrays/maps `*[K:V]`
5. **Type declarations as definitions**: types are first-class values bound with normal definition syntax

After this phase:

```we
# Alias (structural type) — matches any u32
alias: 'u32

# Identifier (nominal type) — only matches values tagged as *u32
identifier: *u32

# Function type
transform: *(u32|string)

# Compound types
point: *{x: f64, y: f64}
scores: *[string: u32]

# Type declaration as a definition (runs at compile time)
MyPoint: *{x: f64, y: f64}

# Using a type in a definition
origin: MyPoint {x: 0.0, y: 0.0}
```

## Background

### ML-Style Structural vs. Nominal Types

welang's type system draws directly from the ML family:

- **Structural types (aliases, `'`)**: Two types are equivalent if they have the same structure. `'u32` matches any 32-bit unsigned integer, regardless of what it's called. This corresponds to ML's `type` declarations (type aliases) and is the foundation of **structural subtyping** (also known as row polymorphism in the context of record types).

- **Nominal types (identifiers, `*`)**: Two types are equivalent only if they have the same name. Even if two nominal types have identical structure, they are distinct. This corresponds to ML's `datatype` declarations (generative types). A value must be explicitly constructed as a given nominal type to satisfy it.

The `'` sigil suggests "a tick/alias" and `*` suggests "starred/identified" — `*` is like `'`, but "more so" (nominal is stricter than structural).

### Type Declarations as Functions

In welang, type declarations are ordinary definitions:

```we
UserId: *u32
Point: *{x: f64, y: f64}
```

These are **run at compile time** — the type is resolved and checked during compilation. In a conditional map (pattern matching), type declarations serve as **type patterns** for dispatch. This will be fully realized in Phase 9 (Pattern Matching); for now, we just parse the type syntax.

### Built-in Type Names

The following are recognized as primitive type names (labels that appear after `*` or `'`):

| Type name | Description |
|-----------|-------------|
| `u8`, `u16`, `u32`, `u64` | Unsigned integers |
| `i8`, `i16`, `i32`, `i64` | Signed integers |
| `f32`, `f64` | Floating-point |
| `string` | String |
| `bool` | Boolean (true/false) |
| `unit` | Unit type (the type of `()`) |

These are not keywords — they are ordinary labels. The type checker (Phase 7) will give them special meaning. The parser treats them as labels.

## Project Context

### Files to Modify

```
Sources/WeLangLib/
    AST.swift        ← add type expression AST nodes
    Parser.swift     ← parse type annotations
    Errors.swift     ← add type-parsing error cases
Tests/WeLangTests/
    ASTTests.swift   ← type expression equality tests
    ParserTests.swift ← type annotation parsing tests
```

### Current Relevant Token Types

From Phase 1, the lexer already produces:

- `.star` — the `*` sigil for nominal types
- `.tick` — the `'` sigil for structural types
- `.leftParen`, `.rightParen` — for function types
- `.leftBrace`, `.rightBrace` — for tuple types
- `.leftBracket`, `.rightBracket` — for array types
- `.pipe` — for separating input/output in function types
- `.label(String)` — type names

## AST Additions

### Type Expression Node

Define a new type for type annotations, separate from `Expr`:

```swift
/// A type expression in the welang type system.
public indirect enum TypeExpr: Equatable {
    /// A named type reference: `u32`, `string`, `MyType`
    case named(String, Span)

    /// A function type: `(inType | outType)`
    case function(input: TypeExpr, output: TypeExpr, Span)

    /// A tuple/object type: `{label: Type, ...}`
    case tupleType(entries: [TypeField], Span)

    /// An array/map type: `[KeyType: ValueType]`
    case arrayType(key: TypeExpr, value: TypeExpr, Span)

    /// Unit type: `()`
    case unitType(Span)
}
```

### Type Field (for tuple types)

```swift
/// A field in a tuple/object type declaration.
public struct TypeField: Equatable {
    public let label: String
    public let type: TypeExpr
    public let span: Span
    public init(label: String, type: TypeExpr, span: Span) { ... }
}
```

### Updated Expr Cases

Add type annotation cases to `Expr`:

```swift
public indirect enum Expr: Equatable {
    // ... existing cases ...

    /// Alias type annotation: `'u32`, `'(u32|string)`, `'{x: f64}`
    case aliasType(TypeExpr, Span)

    /// Identifier (nominal) type annotation: `*u32`, `*(u32|string)`, `*{x: f64}`
    case identifierType(TypeExpr, Span)

    /// Typed expression: a value annotated with a type.
    /// `MyPoint {x: 0.0, y: 0.0}` → .typed(type: .name("MyPoint"), value: .tuple(...))
    case typed(type: Expr, value: Expr, Span)
}
```

### The `typed` Expression

When a name (type reference) is followed by a value expression, it forms a **typed expression**. This is how nominal types are constructed:

```we
origin: MyPoint {x: 0.0, y: 0.0}
#       ^^^^^   ^^^^^^^^^^^^^^^^
#       type    value
```

This creates a value of nominal type `MyPoint`. The parser recognizes this as: a name that looks like a type reference (previously defined as a type) followed by a value. Since we cannot know at parse time whether a name is a type, the **parser should use a heuristic**: if a `.label` is immediately followed by `{`, `[`, `(`, or a literal (without a `:` between them, which would make it a definition), it is parsed as a typed expression.

**Alternatively** (and more cleanly): only parse `.typed` when the label is at the **value position** of a definition and the next token starts a compound literal. This avoids ambiguity with function application.

For this phase, the simplest approach: do **not** parse `.typed` expressions yet. Just parse the type annotation syntax (`*` and `'` prefixed types). The `.typed` construct will be added in Phase 7 when the type checker can disambiguate. For now, focus on type expression syntax.

## Parsing Rules

### Type Annotation as Expression

When the parser encounters `*` or `'` in expression position, it parses a type annotation:

```
TypeAnnotationExpr = "*" TypeExpr   → Expr.identifierType
                   | "'" TypeExpr   → Expr.aliasType
```

### TypeExpr Parsing

```
TypeExpr = Label                           → TypeExpr.named
         | "(" ")"                         → TypeExpr.unitType
         | "(" TypeExpr "|" TypeExpr ")"   → TypeExpr.function
         | "{" TypeFieldList "}"            → TypeExpr.tupleType
         | "[" TypeExpr ":" TypeExpr "]"   → TypeExpr.arrayType
```

#### Named Type

A label immediately following `*` or `'`:

```we
*u32     → .identifierType(.named("u32"))
'string  → .aliasType(.named("string"))
```

#### Function Type

A parenthesized pair separated by `|`:

```we
*(u32|string)   → .identifierType(.function(input: .named("u32"), output: .named("string")))
'(i32|f64)      → .aliasType(.function(input: .named("i32"), output: .named("f64")))
```

Function types can nest:

```we
*((u32|string)|(string|bool))
# A function from (u32→string) to (string→bool)
```

#### Tuple Type

Braces with labeled type fields:

```we
*{x: f64, y: f64}  → .identifierType(.tupleType(entries: [("x", .named("f64")), ("y", .named("f64"))]))
```

#### Array Type

Brackets with key-type and value-type:

```we
*[string: u32]  → .identifierType(.arrayType(key: .named("string"), value: .named("u32")))
```

### Integration with Expression Parsing

In `parseAtom()` (or `parsePostfixExpr()`), add cases for `.star` and `.tick`:

```swift
case .star:
    advance()  // consume *
    let typeExpr = try parseTypeExpr()
    return .identifierType(typeExpr, span)

case .tick:
    advance()  // consume '
    let typeExpr = try parseTypeExpr()
    return .aliasType(typeExpr, span)
```

`parseTypeExpr()` is a new parsing function that handles the type expression grammar above.

### Disambiguation

- `*` and `'` in expression position always start a type annotation. They cannot appear in any other context (they are not used as operators).
- Inside a type expression, `|` is always the function type separator (not the pipe combinator). This is unambiguous because type expressions are a distinct syntactic context.
- `()` in a type expression is the unit type, not an empty S-expression.

## Error Cases

Add to `ParseError`:

```swift
case expectedTypeName(span: Span)          // * or ' not followed by valid type
case expectedTypeOutputType(span: Span)    // function type missing output after |
case expectedTypeFieldLabel(span: Span)    // tuple type field missing label
case expectedTypeFieldType(span: Span)     // tuple type field missing type after :
case expectedTypeValueType(span: Span)     // array type missing value type after :
```

## Tests to Write

### AST Tests

- `testTypeExprNamedEquality`: `.named("u32") == .named("u32")`
- `testTypeExprNamedInequality`: `.named("u32") != .named("i32")`
- `testTypeExprFunctionEquality`: function types with same in/out
- `testTypeExprTupleTypeEquality`: same fields
- `testTypeExprArrayTypeEquality`: same key/value types
- `testTypeFieldEquality`: same label and type

### Parser Tests

**Alias types:**
- `testParseAliasNamedType`: `"t: 'u32"` → `.aliasType(.named("u32"))`
- `testParseAliasFunctionType`: `"t: '(u32|string)"` → alias wrapping function type
- `testParseAliasUnitType`: `"t: '()"` → alias wrapping unit type
- `testParseAliasTupleType`: `"t: '{x: f64, y: f64}"` → alias wrapping tuple type
- `testParseAliasArrayType`: `"t: '[string: u32]"` → alias wrapping array type

**Identifier types:**
- `testParseIdentifierNamedType`: `"t: *u32"` → `.identifierType(.named("u32"))`
- `testParseIdentifierFunctionType`: `"t: *(u32|string)"` → identifier wrapping function type
- `testParseIdentifierTupleType`: `"t: *{x: f64, y: f64}"` → identifier wrapping tuple type
- `testParseIdentifierArrayType`: `"t: *[string: u32]"` → identifier wrapping array type

**Nested function types:**
- `testParseNestedFunctionType`: `"t: *((u32|string)|(string|bool))"` → nested function types

**Type as definition value:**
- `testParseTypeDefinition`: `"UserId: *u32"` → definition with `.identifierType` value
- `testParseMultipleTypeDefinitions`:
  ```
  Point: *{x: f64, y: f64}
  Transform: *(Point|Point)
  ```
  → two type definitions

**Types in expressions:**
- `testParseTypeInSExpr`: `"r: (foo *u32)"` → type annotation as argument in apply
- `testParseTypeInPipe`: `"r: (x | *u32)"` → type annotation in pipe clause

**Error cases:**
- `testParseStarWithoutType`: `"t: *"` followed by newline → throws `ParseError.expectedTypeName`
- `testParseTickWithoutType`: `"t: '"` followed by newline → throws `ParseError.expectedTypeName`
- `testParseFunctionTypeMissingOutput`: `"t: *(u32|)"` → throws error
- `testParseTupleTypeMissingFieldType`: `"t: *{x:}"` → throws error

### Compile Tests

- `testCompileTypeDefinition`: `"T: *u32"` compiles without error
- `testCompileAliasDefinition`: `"T: 'u32"` compiles without error

## Success Criteria

1. `swift build` compiles without errors.
2. `swift test` — full suite passes.
3. Alias and identifier type annotations parse correctly for all forms (named, function, tuple, array, unit).
4. Type annotations work as expression values in definitions, S-expressions, and pipes.
5. Nested type expressions (function types with compound arguments) parse correctly.
6. All error cases produce appropriate `ParseError` variants.

## Important Notes

- **Types are first-class values**: A type annotation like `*u32` is an expression that evaluates to a type value at compile time. The parser treats it as an expression — the type checker (Phase 7) will validate and evaluate it.
- **No `.typed` expressions yet**: We parse the type syntax but do not yet parse the pattern of `TypeName value`. That will come when the type checker can disambiguate.
- **`|` inside type expressions is not a pipe**: The parser must distinguish between the pipe combinator (inside S-expressions) and the function type separator (inside type expressions). The context makes this unambiguous — type expressions are entered via `*` or `'`.
- Keep all types `public` and `Equatable`.
- Run `swift test` before considering this phase complete.
