# Phase 4: Compound Data Types — Tuples, Objects, Arrays, Maps, and Access

## Goal

Implement parsing for welang's compound data structures and their access patterns:

1. **Tuples/Objects** (`{}`): fixed-size, heterogeneously-typed collections with compile-time-known keys
2. **Arrays/Maps** (`[]`): dynamically-keyed, homogeneously-typed collections
3. **Member access** (`.`): tuple/object field access
4. **Index access** (`[]` postfix): array/map element access

After this phase, the parser can handle:

```we
implicitIndex: {1, 0.1}
explicitIndex: {0: 1, 2: 0.1}
explicitLabels: {label: 1, other: 0.1}
explicitMap: {"key": "value"}

implicitArray: [12, 24]
explicitArray: [1: 12, 3: 24]
labeledArray: [key: 12, other: 24]
stringKeyed: ["some string": 12, "other": 24]

accessor: (x.label)
indexer: (x[0])
nested: (x.[ x.[1] ])
```

## Background

### Tuples/Objects (ML Record Types)

Curly-brace literals `{}` define **product types** (tuples / records / objects). They are structurally typed — two values with the same field layout are the same type. This follows the ML tradition of record types.

Key properties:
- **All keys and value types are known at compile time** (this is the defining difference from arrays)
- Keys can be: **implicit integer indices** (auto-assigned 0, 1, 2...), **explicit integer indices**, **labels**, or **string keys**
- Different entries can have different value types (heterogeneous)
- `{1, 0.1}` has implicit indices: `{0: 1, 1: 0.1}`

### Arrays/Maps (Homogeneous Collections)

Square-bracket literals `[]` define **collections** that are dynamically indexable at runtime. They sacrifice type heterogeneity for dynamic keying:

- **Single value type** for all entries (homogeneous)
- Keys can be: implicit integer indices, explicit integers, labels, or strings
- Dynamic subscript access at runtime

### Access Patterns

- **Dot access** (`x.label`): compile-time field access on tuples/objects
- **Bracket access** (`x[0]`, `x["key"]`): runtime index access on arrays/maps
- **Computed access** (`x.[ expr ]`): the expression inside `.[]` is evaluated to produce the key

## Project Context

### Files to Modify

```
Sources/WeLangLib/
    AST.swift        ← add compound literal and access Expr cases
    Parser.swift     ← extend expression parsing for { }, [ ], .field, [index]
    Errors.swift     ← add parse error cases as needed
Tests/WeLangTests/
    ASTTests.swift   ← tests for new AST nodes
    ParserTests.swift ← comprehensive tests
```

### Current Expr Enum (from Phase 3)

```swift
public indirect enum Expr: Equatable {
    case integerLiteral(String, Span)
    case floatLiteral(String, Span)
    case stringLiteral(String, Span)
    case interpolatedStringLiteral(String, Span)
    case name(String, Span)
    case discard(Span)
    case unit(Span)
    case apply(function: Expr, arguments: [Expr], Span)
    case pipe(clauses: [Expr], Span)
}
```

## AST Additions

### Compound Literal Key Types

Define an enum for the different key forms in compound literals:

```swift
/// A key in a tuple/object or array/map entry.
public enum CompoundKey: Equatable {
    /// Implicitly assigned sequential integer index (no explicit key written).
    case implicit

    /// Explicit integer index: `{0: value}` or `[1: value]`
    case index(String, Span)

    /// Label key: `{label: value}` or `[label: value]`
    case label(String, Span)

    /// String key: `{"key": value}` or `["key": value]`
    case stringKey(String, Span)
}
```

### Compound Entry

```swift
/// A single key-value entry in a compound literal.
public struct CompoundEntry: Equatable {
    public let key: CompoundKey
    public let value: Expr
    public let span: Span

    public init(key: CompoundKey, value: Expr, span: Span) { ... }
}
```

### New Expr Cases

Add to the `Expr` enum:

```swift
public indirect enum Expr: Equatable {
    // ... existing cases ...

    /// Tuple/object literal: `{1, 0.1}`, `{label: 1}`, `{"key": "value"}`
    case tuple(entries: [CompoundEntry], Span)

    /// Array/map literal: `[12, 24]`, `[key: 12]`, `["k": 1]`
    case array(entries: [CompoundEntry], Span)

    /// Dot access on a tuple/object: `x.label`, `x.0`
    case dotAccess(expr: Expr, field: String, Span)

    /// Bracket index access: `x[0]`, `x["key"]`
    case bracketAccess(expr: Expr, index: Expr, Span)

    /// Computed dot-bracket access: `x.[ expr ]`
    case computedAccess(expr: Expr, index: Expr, Span)
}
```

## Parsing Rules

### Compound Literals

#### Tuple / Object (`{...}`)

```
TupleLiteral = "{" EntryList? "}"
EntryList    = Entry ("," Entry)* ","?    # trailing comma is optional
Entry        = (Key ":")? Expr
Key          = IntegerLiteral | Label | StringLiteral
```

Parsing logic:
1. Consume `{`
2. If immediately `}`, return `.tuple(entries: [], span)`
3. Parse entries separated by commas (skip newlines inside braces, same as parens)
4. For each entry: **look ahead** to determine if there is a key
   - If current token is `.integerLiteral`, `.label`, or `.stringLiteral`, AND the next token is `.colon`, then it is a keyed entry — consume key, consume `:`, parse value
   - Otherwise, it is an implicit-keyed entry — parse value only, assign `.implicit`
5. Consume `}`

**Disambiguation**: The tricky case is `{foo: 1}` vs `{foo, 1}`. When you see a label followed by `:`, it is a keyed entry. When you see a label followed by `,` or `}`, it is a value (a name reference) with an implicit key.

#### Array / Map (`[...]`)

```
ArrayLiteral = "[" EntryList? "]"
EntryList    = Entry ("," Entry)* ","?
Entry        = (Key ":")? Expr
Key          = IntegerLiteral | Label | StringLiteral
```

Same parsing logic as tuples but with `[` and `]` delimiters, producing `.array(...)`.

**Newlines inside braces and brackets are insignificant** (whitespace), same as parentheses.

### Access Expressions

Access is a **postfix** operation on an atom. After parsing an atom, check for trailing access:

```
PostfixExpr = Atom Accessor*
Accessor    = "." Label
            | "." IntegerLiteral
            | "." "[" Expr "]"     # computed access
            | "[" Expr "]"          # bracket access
```

Update the atom parsing to:

1. Parse the base atom (literal, name, paren, brace, bracket)
2. Loop: while the next token is `.dot` or `.leftBracket`:
   - **Dot followed by label**: `.dotAccess(expr, label, span)`
   - **Dot followed by integer**: `.dotAccess(expr, integerText, span)`
   - **Dot followed by `[`**: parse inner expression, consume `]` → `.computedAccess(expr, index, span)`
   - **`[` (no preceding dot)**: parse inner expression, consume `]` → `.bracketAccess(expr, index, span)`

### Integration with S-Expressions and Pipes

Compound literals and access expressions are **atoms** — they can appear anywhere an atom can appear:

```we
# Tuple as argument in S-expression
result: (merge {a: 1} {b: 2})

# Array in pipe
result: ([1, 2, 3] | sum)

# Access in pipe clause
result: (x | x.name | toUpper)
```

Within a clause (inside parens), after parsing each atom, check for postfix access. This means `parseAtom()` should be renamed or wrapped to become `parsePostfixExpr()`, which handles the base atom plus any trailing `.field` or `[index]` chains.

### Empty Compound Literals

- `{}` is an empty tuple (valid)
- `[]` is an empty array (valid)

### Disambiguation: Array Literal vs. Bracket Access

When `[` appears, context determines whether it is:
- A **new array literal** (when at the start of an expression, or after `:`, `,`, `|`, `(`, etc.)
- A **bracket access** (when immediately after an atom with no whitespace, or contextually as a postfix)

In practice, within the parser:
- In `parseAtom()`: `[` starts an array literal
- In the postfix loop after an atom: `[` is bracket access

This is handled naturally by the two-phase parsing (atom then postfix).

## Error Cases

Add to `ParseError` as needed:

- `expectedClosingBrace(span: Span)`: missing `}`
- `expectedClosingBracket(span: Span)`: missing `]`
- `expectedField(span: Span)`: dot not followed by a valid field name

## Tests to Write

### AST Tests

- `testTupleEquality`: two `.tuple` with same entries are equal
- `testArrayEquality`: two `.array` with same entries are equal
- `testDotAccessEquality`: same base and field
- `testBracketAccessEquality`: same base and index
- `testComputedAccessEquality`: same base and computed index
- `testCompoundEntryEquality`: same key and value
- `testCompoundKeyImplicit`: `.implicit == .implicit`
- `testCompoundKeyLabel`: `.label("a", _) == .label("a", _)`

### Parser Tests

**Tuple/Object literals:**
- `testParseTupleImplicitKeys`: `"t: {1, 0.1}"` → tuple with two implicit-keyed entries
- `testParseTupleExplicitIntKeys`: `"t: {0: 1, 2: 0.1}"` → tuple with integer keys
- `testParseTupleLabelKeys`: `"t: {label: 1, other: 0.1}"` → tuple with label keys
- `testParseTupleStringKeys`: `"t: {\"key\": \"value\"}"` → tuple with string keys
- `testParseTupleMixed`: `"t: {a: 1, 2}"` → first entry keyed, second implicit
- `testParseEmptyTuple`: `"t: {}"` → empty tuple
- `testParseTupleTrailingComma`: `"t: {1, 2,}"` → valid, two entries
- `testParseNestedTuple`: `"t: {a: {b: 1}}"` → nested tuple

**Array/Map literals:**
- `testParseArrayImplicitKeys`: `"a: [12, 24]"` → array with implicit keys
- `testParseArrayExplicitIntKeys`: `"a: [1: 12, 3: 24]"` → array with integer keys
- `testParseArrayLabelKeys`: `"a: [key: 12, other: 24]"` → array with label keys
- `testParseArrayStringKeys`: `"a: [\"some\": 12, \"other\": 24]"` → string-keyed
- `testParseEmptyArray`: `"a: []"` → empty array
- `testParseNestedArray`: `"a: [[1, 2], [3, 4]]"` → nested arrays

**Access expressions:**
- `testParseDotAccessLabel`: `"r: (x.label)"` → `.dotAccess(.name("x"), "label")`
- `testParseDotAccessIndex`: `"r: (x.0)"` → `.dotAccess(.name("x"), "0")`
- `testParseBracketAccess`: `"r: (x[0])"` → `.bracketAccess(.name("x"), .integerLiteral("0"))`
- `testParseBracketAccessString`: `"r: (x[\"key\"])"` → bracket access with string
- `testParseComputedAccess`: `"r: (x.[ x.[1] ])"` → computed access with nested access
- `testParseChainedDotAccess`: `"r: (x.a.b.c)"` → chained dot access (three levels)
- `testParseAccessInPipe`: `"r: ({k: 2} | x.k)"` → pipe with dot access in second clause

**Multi-line compound literals:**
- `testParseMultilineTuple`: tuple spanning multiple lines
- `testParseMultilineArray`: array spanning multiple lines

**Error cases:**
- `testParseUnclosedTuple`: `"t: {1, 2"` → throws `ParseError.expectedClosingBrace`
- `testParseUnclosedArray`: `"a: [1, 2"` → throws `ParseError.expectedClosingBracket`
- `testParseDotWithoutField`: `"r: (x.)"` → throws `ParseError.expectedField`

### Compile Tests

- `testCompileTupleLiteral`: `"t: {1, 2}"` compiles without error
- `testCompileArrayLiteral`: `"a: [1, 2]"` compiles without error

## Success Criteria

1. `swift build` compiles without errors.
2. `swift test` — full suite passes.
3. All four compound literal forms (implicit, explicit int, label, string key) parse correctly for both tuples and arrays.
4. Dot access, bracket access, and computed access parse and chain correctly.
5. Compound literals work as atoms within S-expressions and pipes.
6. Multi-line compound literals (newlines inside `{}` and `[]`) parse correctly.

## Important Notes

- **Tuples are heterogeneous, arrays are homogeneous** — but the parser doesn't enforce this. Type checking (Phase 7) will validate that all array values share a type.
- **Newlines inside `{}`, `[]`, and `()` are all insignificant**. Maintain a nesting depth counter that covers all three bracket types.
- Keep all types `public` and `Equatable`.
- Run `swift test` before considering this phase complete.
