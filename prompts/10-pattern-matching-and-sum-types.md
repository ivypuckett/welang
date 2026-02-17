# Phase 10: Pattern Matching and Sum Types

## Goal

Implement welang's pattern matching via **conditional maps** and **discriminated unions** (sum types / tagged unions). This phase adds:

1. **Conditional maps**: array-like syntax where keys are predicate functions and values are results — welang's pattern matching construct
2. **Discriminated unions (sum types)**: types that can be one of several variants, following ML's `datatype` tradition
3. **Exhaustiveness checking**: ensuring all variants of a sum type are handled
4. **Type declarations in conditional maps**: type-based dispatch (runtime type patterns)

After this phase:

```we
# Conditional map — pattern matching on predicates
isOfAge: [
  (x.greaterThan 20): true
  (_): false
]

# Sum type definition
Shape: *{
  circle: *{radius: f64},
  rectangle: *{width: f64, height: f64}
}

# Pattern matching on sum type
area: [
  (x.is *circle): (multiply 3.14159 (multiply x.radius x.radius))
  (x.is *rectangle): (multiply x.width x.height)
]

# Simple conditional
abs: [
  (x.greaterThan 0): x
  (_): (negate x)
]
```

## Background

### Conditional Maps (welang's Pattern Matching)

In welang, pattern matching uses the same square-bracket `[]` syntax as arrays, but with a different semantic: the **key** is a predicate function (evaluated with the input), and the **value** is the result if the predicate returns true.

```we
isOfAge: [
  (x.greaterThan 20): true
  (_): false
]
```

This is equivalent to:
```
if greaterThan(x, 20) then true
else false
```

The `_` (discard) pattern is a **wildcard** — it always matches. It must appear last and serves as the default case.

Conditional maps are evaluated **top-to-bottom**: the first matching predicate wins. This is the same semantics as ML's `case` expressions and Haskell's guards.

### Discriminated Unions / Sum Types (ML `datatype`)

Sum types in welang are defined as tuple types where each field represents a **variant**:

```we
Shape: *{
  circle: *{radius: f64},
  rectangle: *{width: f64, height: f64}
}
```

This defines `Shape` as a type with two variants: `circle` (carrying a record with `radius`) and `rectangle` (carrying a record with `width` and `height`). This corresponds to:

- **ML**: `datatype shape = Circle of {radius: real} | Rectangle of {width: real, height: real}`
- **Haskell**: `data Shape = Circle {radius :: Double} | Rectangle {width :: Double, height :: Double}`
- **Rust**: `enum Shape { Circle { radius: f64 }, Rectangle { width: f64, height: f64 } }`

The key difference from a regular tuple: a sum type value is **exactly one** of its variants at runtime. Construction specifies the variant:

```we
myCircle: Shape.circle {radius: 5.0}
```

### Type Patterns in Conditional Maps

When a conditional map appears with type-checking predicates, it acts as a type-level `match`:

```we
area: [
  (x.is *circle): (computeCircleArea x)
  (x.is *rectangle): (computeRectArea x)
]
```

The `is` built-in checks the variant tag at runtime. Within the body of a matched branch, the type checker **narrows** the type of `x` to the matched variant (like TypeScript's type guards or Kotlin's smart casts).

### Decision Trees

For compilation efficiency, pattern matches are compiled to **decision trees** (also called case trees). This is the standard compilation strategy for ML pattern matching (see Maranget, 2008: "Compiling Pattern Matching to Good Decision Trees"). However, for this phase, a linear scan (evaluate predicates top-to-bottom) is sufficient. Decision tree optimization can come later.

## Project Context

### Files to Create/Modify

```
Sources/WeLangLib/
    AST.swift            ← add conditional map and sum type AST nodes
    Parser.swift         ← parse conditional map syntax
    TypeInference.swift  ← type-check conditionals, sum types, exhaustiveness
    Errors.swift         ← add pattern matching errors
Tests/WeLangTests/
    ASTTests.swift       ← conditional map and sum type node tests
    ParserTests.swift    ← conditional map parsing tests
    TypeInferenceTests.swift ← pattern matching type inference tests
```

## AST Additions

### Conditional Map Entry

```swift
/// A branch in a conditional map (pattern match).
public struct ConditionalBranch: Equatable {
    /// The pattern/predicate expression.
    /// For type patterns: `(x.is *circle)`
    /// For predicate patterns: `(x.greaterThan 20)`
    /// For wildcard: `_`
    public let pattern: Expr

    /// The result expression if the pattern matches.
    public let body: Expr

    public let span: Span

    public init(pattern: Expr, body: Expr, span: Span) { ... }
}
```

### New Expr Case

```swift
public indirect enum Expr: Equatable {
    // ... existing cases ...

    /// Conditional map (pattern matching):
    /// ```
    /// [
    ///   (predicate1): result1
    ///   (predicate2): result2
    ///   (_): default
    /// ]
    /// ```
    case conditionalMap(branches: [ConditionalBranch], Span)
}
```

## Parsing

### Disambiguating `[` — Array vs. Conditional Map

The `[` token can start either an array/map literal (Phase 4) or a conditional map. The disambiguation rule:

- If the first entry has a **parenthesized expression or discard as the key** followed by `:`, it is a conditional map.
- If the first entry has an **integer, label, or string as the key** followed by `:`, or no key at all (implicit index), it is an array/map literal.

More precisely:

```
"[" "(" ... ")" ":" ...    → conditional map
"[" "_" ":" ...             → conditional map
"[" IntLiteral ":" ...      → array literal
"[" Label ":" ...           → array literal (label-keyed)
"[" StringLiteral ":" ...   → array literal (string-keyed)
"[" Expr "," ...            → array literal (implicit keys)
```

The key insight: conditional map keys are **always** wrapped in parentheses `()` or are `_` (discard). Array keys are never parenthesized.

### Conditional Map Parsing

```
ConditionalMap = "[" (ConditionalBranch Newline*)+ "]"
ConditionalBranch = Pattern ":" Expr
Pattern = "(" Expr ")"
        | "_"
```

Inside `[...]`, when the parser sees `(` or `_` at the start of an entry, it switches to conditional map mode:

1. Parse the pattern: either a parenthesized expression or `_`
2. Consume `:`
3. Parse the body expression
4. Continue until `]`

Branches are separated by newlines (not commas, unlike array entries). This is a syntactic distinction from arrays.

### Newline Handling in Conditional Maps

Inside conditional maps, newlines serve as branch separators (similar to how they separate definitions at the top level). This means **newlines are significant** inside conditional maps, unlike inside regular `[]` array literals where they are insignificant.

To handle this:
- When parsing a `[` that turns out to be a conditional map, treat newlines as branch separators.
- Skip blank lines (multiple consecutive newlines).

### Sum Type Construction

Sum type construction uses dot access on a type name:

```we
myCircle: Shape.circle {radius: 5.0}
```

This is already parseable as `Shape.circle` (dot access on a name) followed by a tuple literal. The type checker will interpret this as variant construction. No new parsing is needed.

### The `is` Built-in

The pattern `(x.is *typename)` is parsed as:
- `x.is` — dot access on `x`, accessing field `is`
- `*typename` — an identifier type argument

This is already parseable with existing syntax. The type checker will recognize `is` as a special built-in operation for variant checking.

## Type Inference for Conditional Maps

### All Branches Must Return the Same Type

The type of a conditional map is the return type shared by all branches:

```swift
case .conditionalMap(let branches, _):
    let resultType = gen.fresh()
    var s = Substitution()

    for branch in branches {
        // Type-check the pattern (must return bool)
        let (s1, patType) = try w(env: s.apply(env), expr: branch.pattern, gen: &gen)
        s = s.compose(with: s1)

        // Pattern must be bool (or discard, which always matches)
        if !isDiscard(branch.pattern) {
            let s2 = try unify(s1.apply(patType), .primitive(.bool))
            s = s.compose(with: s2)
        }

        // Type-check the body
        let (s3, bodyType) = try w(env: s.apply(env), expr: branch.body, gen: &gen)
        s = s.compose(with: s3)

        // Body type must unify with result type
        let s4 = try unify(s.apply(resultType), s3.apply(bodyType))
        s = s.compose(with: s4)
    }

    return (s, s.apply(resultType))
```

### Type Narrowing

When a branch pattern is `(x.is *circle)`, within that branch's body, `x` should be narrowed to the `circle` variant. Implement this by:

1. Detecting `is` patterns during type inference
2. Creating a narrowed environment for the branch body where `x` has the more specific type

```swift
// If the pattern is of the form (x.is *TypeName):
// Narrow x's type in the body environment to TypeName's variant type
```

### Exhaustiveness Checking

For sum types, all variants must be handled. The type checker should verify:

1. If the conditional map matches against a sum type's variants, **every variant must have a branch**, OR a wildcard `_` branch must be present.
2. If no wildcard and not all variants are covered, emit a warning or error.

Exhaustiveness checking algorithm:
1. Collect all variant names from the matched sum type.
2. Collect all variant names mentioned in `(x.is *variant)` patterns.
3. If there is a `_` branch, exhaustiveness is satisfied.
4. Otherwise, the missing variants are: `allVariants - matchedVariants`.
5. If missing is non-empty, throw `TypeError.nonExhaustiveMatch(missing:)`.

## Error Cases

```swift
// In TypeError:
case nonExhaustiveMatch(missing: [String], Span)
case branchTypeMismatch(expected: Type, got: Type, Span)

// In ParseError:
case expectedConditionalBranch(span: Span)
case expectedPatternColon(span: Span)
```

## Tests to Write

### AST Tests

- `testConditionalBranchEquality`: same pattern and body
- `testConditionalBranchInequality`: different pattern or body
- `testConditionalMapEquality`: same branches
- `testConditionalMapInequality`: different branches

### Parser Tests

**Basic conditional maps:**
- `testParseSimpleConditionalMap`:
  ```
  isPositive: [
    (x.greaterThan 0): true
    (_): false
  ]
  ```
  → `.conditionalMap` with two branches

- `testParseConditionalMapSingleBranch`: `"f: [(_): 0]"` → one branch with wildcard

- `testParseConditionalMapMultipleBranches`:
  ```
  classify: [
    (x.greaterThan 100): "large"
    (x.greaterThan 10): "medium"
    (_): "small"
  ]
  ```
  → three branches

**Type patterns:**
- `testParseTypePattern`:
  ```
  area: [
    (x.is *circle): (computeCircleArea x)
    (x.is *rectangle): (computeRectArea x)
  ]
  ```
  → branches with type-checking patterns

**Disambiguation:**
- `testParseArrayNotConditionalMap`: `"a: [1, 2, 3]"` → still parsed as array
- `testParseLabeledArrayNotConditionalMap`: `"a: [key: 1]"` → still parsed as labeled array

**Error cases:**
- `testParseMissingPatternColon`: pattern without `:` → error
- `testParseEmptyConditionalMap`: `"f: []"` → parsed as empty array (not conditional map)

### Type Inference Tests

**Conditional map type inference:**
- `testInferConditionalMapSameReturnType`: all branches return same type → succeeds
- `testInferConditionalMapMismatchedReturnTypes`: branches return different types → `branchTypeMismatch` error
- `testInferConditionalMapPredicateNotBool`: pattern doesn't return bool → type mismatch

**Sum type tests:**
- `testInferSumTypeDefinition`: define a sum type → type is correctly recorded
- `testInferSumTypeConstruction`: construct a variant → type is the sum type
- `testInferSumTypePatternMatch`: match on variants → type narrowing works

**Exhaustiveness tests:**
- `testExhaustivenessAllVariantsCovered`: all variants matched → passes
- `testExhaustivenessWithWildcard`: wildcard present → passes
- `testExhaustivenessMissingVariant`: variant not matched, no wildcard → `nonExhaustiveMatch` error

### Compile Tests

- `testCompileConditionalMap`: simple conditional map compiles
- `testCompileSumTypeMatch`: sum type pattern match compiles

## Success Criteria

1. `swift build` compiles without errors.
2. `swift test` — full suite passes.
3. Conditional maps parse correctly, distinct from array literals.
4. All branches of a conditional map must return the same type.
5. Predicate patterns must evaluate to booleans (or be wildcards).
6. Sum type variants can be matched with `(x.is *variant)` patterns.
7. Type narrowing works within matched branches.
8. Exhaustiveness checking catches missing variants.

## Important Notes

- **Conditional maps use newline separators, not commas**: this is the key syntactic difference from arrays.
- **The `_` wildcard is the catch-all**: it must be last. If `_` is not last, later branches are unreachable — emit a warning.
- **Sum types are defined as nominal tuple types**: `*{variant1: type1, variant2: type2}`. The nominal tag makes them distinct from plain tuples.
- **Decision tree compilation is not needed yet**: a linear top-to-bottom evaluation is sufficient for correctness. Optimization via decision trees (à la Maranget) is a future enhancement.
- **`is` is not a keyword**: it's looked up as a built-in via dot access. This keeps the language keyword-free.
- Keep all types `public` and `Equatable`.
- Run `swift test` before considering this phase complete.
