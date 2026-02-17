# Phase 8: Higher-Order Functions and Parametric Polymorphism

## Goal

Extend welang's type system and runtime semantics to fully support:

1. **Higher-order functions**: functions that accept and/or return other functions
2. **Parametric polymorphism**: every function is naturally polymorphic (as in ML), and users can add explicit type constraints
3. **Currying**: multi-argument application is desugared into nested single-argument calls
4. **Closures**: functions that capture variables from their enclosing scope
5. **Typed function definitions**: explicit type annotations on function signatures

After this phase:

```we
# Higher-order: map takes a function and an array
map: (f | x | applyToEach f x)

# Naturally polymorphic — works for any type
identity: x

# Explicit type constraint
intId: *i64 x

# Function passed as argument
doubled: (map double [1, 2, 3])

# Composition via pipes
transform: (| double | increment)

# Function type in alias
Transformer: '(i64|i64)
myTransform: Transformer (| double)
```

## Background

### Parametric Polymorphism (ML Heritage)

In the ML tradition, every `let`-bound function is **automatically polymorphic**. The function `identity: x` has type `∀α. α → α` — it works for any type. This is called **let-polymorphism** or **prenex polymorphism** (Rank-1 polymorphism in the Hindley-Milner system).

Phase 7 implemented the core Algorithm W with let-generalization. This phase extends that foundation with:

- **Higher-order function types**: `'(('(a|b))|('(b|c))|('(a|c)))` — a function that takes two functions and returns their composition
- **Explicit type annotations**: constraining a polymorphic function to a specific type
- **Closure capture**: when a function references names from an outer scope

### Currying (ML Convention)

welang functions are **always monadic** — they take exactly one argument (`x`) and return one value. Multi-argument functions are encoded via **currying** (named after Haskell Curry, standard in ML):

```we
# `add` takes one argument and returns a function that takes another
add: (addImpl x)
# When called as (add 1 2), this is ((add 1) 2):
#   (add 1) → returns a function expecting the second argument
#   that function applied to 2 → returns the result
```

The parser already represents `(f a b)` as `.apply(f, [a, b])`. The type checker (Phase 7) already handles this by threading fresh type variables through each argument. This phase ensures the **semantics** are correct for higher-order usage.

### Closures

When a function body references a name from an outer scope, it **captures** that name's value:

```we
offset: 10
addOffset: (add x offset)
# addOffset captures `offset` from the enclosing scope
```

In the type system, closures are just functions. The captured variables are part of the function's **closure environment** — this matters for codegen (Phase 11/12) but at the type level, closures and regular functions are indistinguishable.

## Project Context

### Files to Modify

```
Sources/WeLangLib/
    AST.swift            ← add explicit type annotation on definitions
    TypeInference.swift  ← extend inference for higher-order patterns
    Types.swift          ← ensure function types support nesting
    Errors.swift         ← add relevant error cases
Tests/WeLangTests/
    TypeInferenceTests.swift ← comprehensive higher-order tests
    ParserTests.swift        ← tests for annotated definitions
```

### Current State (from Phase 7)

The type inference engine supports:
- Algorithm W with unification
- Let-polymorphism (generalization/instantiation)
- Inference for literals, names, apply, pipe, tuple, array
- A `TypeScheme` with quantified variables
- A `TypeEnv` mapping names to schemes

## AST Additions

### Explicit Type Annotation on Definitions

Allow definitions to have an optional type annotation:

```swift
public struct Definition: Equatable {
    public let label: String
    public let typeAnnotation: Expr?  // NEW: optional type annotation (e.g., *i64)
    public let value: Expr
    public let span: Span
}
```

This supports syntax like:

```we
# Type-annotated definition
intId: *i64 x

# The parser sees: label "intId", colon, then the value expression
# The value is: *i64 applied to x? Or is *i64 a type annotation on x?
```

**Parsing strategy**: When parsing a definition's value, if the first expression is a type annotation (`*` or `'` prefixed), and it is followed by another expression, treat the type annotation as the definition's type and the rest as the value:

```
Definition = Label ":" TypeAnnotation? Expr
TypeAnnotation = ("*" | "'") TypeExpr
```

If a type annotation is present, store it in `Definition.typeAnnotation`. The type checker validates that the value conforms to the annotation.

### Function Expressions

Currently, a function is implicitly any expression that references `x`. To make higher-order patterns explicit, we may want a way to create anonymous functions (lambdas). In welang, the approach is:

- A definition whose value is an S-expression or pipe that references `x` is a function.
- Functions are first-class values — they can be passed and returned.
- The `(| f | g)` pattern with a leading pipe creates a function: it takes `x` as input and pipes through `f` then `g`.

No new AST nodes are needed for lambdas — the leading pipe `(| ...)` already serves this purpose.

## Type Inference Extensions

### Higher-Order Function Inference

The existing Algorithm W already handles higher-order functions naturally. When a function is passed as an argument, its type is unified with the expected parameter type:

```we
apply: (x.fn x.arg)
# Inferred: ∀α β. {fn: (α → β), arg: α} → β
```

Ensure the type checker handles:

1. **Functions as values in tuples/objects**: `{fn: double, value: 1}` — the `fn` field has a function type.
2. **Functions as arguments**: `(map double [1,2,3])` — `double` is passed to `map`.
3. **Functions as return values**: `(| double)` — returns a function.
4. **Composition**: `(| f | g)` — creates a new function that is the composition of `f` and `g`.

### Type Annotation Checking

When a definition has an explicit type annotation:

```swift
func inferAnnotatedDefinition(env: inout TypeEnv, def: Definition, gen: inout TypeVarGenerator) throws {
    // 1. Infer the value's type using Algorithm W
    let (s, inferredType) = try inferDefinitionValue(env: env, def: def, gen: &gen)

    // 2. If there's a type annotation, resolve it to an internal Type
    if let annotation = def.typeAnnotation {
        let annotationType = try resolveTypeExpr(annotation, env: env)
        // 3. Unify the inferred type with the annotation
        let s2 = try unify(s.apply(inferredType), annotationType)
        let finalSub = s.compose(with: s2)
        let scheme = generalize(env: finalSub.apply(env), type: finalSub.apply(annotationType))
        env = finalSub.apply(env).extending(def.label, with: scheme)
    } else {
        // No annotation — use inferred type (existing behavior)
        let scheme = generalize(env: s.apply(env), type: s.apply(inferredType))
        env = s.apply(env).extending(def.label, with: scheme)
    }
}
```

### Resolving Type Annotations

Convert the AST `TypeExpr` (from type annotations) into the internal `Type` representation:

```swift
func resolveTypeExpr(_ expr: Expr, env: TypeEnv) throws -> Type {
    switch expr {
    case .aliasType(let typeExpr, _):
        return try resolveType(typeExpr)
    case .identifierType(let typeExpr, _):
        let inner = try resolveType(typeExpr)
        return .nominal(name: /* extract name */, inner: inner)
    default:
        throw TypeError.invalidTypeAnnotation(expr)
    }
}

func resolveType(_ typeExpr: TypeExpr) throws -> Type {
    switch typeExpr {
    case .named(let name, _):
        if let prim = PrimitiveType(rawValue: name) {
            return .primitive(prim)
        }
        // Look up user-defined type names
        ...
    case .function(let input, let output, _):
        return .function(try resolveType(input), try resolveType(output))
    case .tupleType(let fields, _):
        return .tuple(try fields.map { ($0.label, try resolveType($0.type)) })
    case .arrayType(let key, let value, _):
        return .array(key: try resolveType(key), value: try resolveType(value))
    case .unitType(_):
        return .unit
    }
}
```

### Subsumption: Structural vs. Nominal

When checking type annotations:

- An **alias** (`'T`) check is **structural**: the value must have compatible structure, regardless of nominal tag.
- An **identifier** (`*T`) check is **nominal**: the value must be tagged with the exact type name.

In unification:
- `'u32` unifies with any type that structurally matches `u32` (which is just `u32` itself for primitives).
- `*MyType` only unifies with another `*MyType` (via the `nominal` case in unification).

### Polymorphism Constraints

welang's polymorphism is standard ML rank-1. Some things to verify:

1. **Value restriction**: In ML, polymorphism is only generalized for syntactic values (not arbitrary expressions). For welang, since every definition is `label: expr`, generalize only when `expr` is a syntactic value (literal, lambda, name reference). For complex expressions, don't generalize — assign a monomorphic type.

2. **Monomorphism of `x`**: The implicit parameter `x` is never polymorphic within a single function body. It gets a fresh type variable that is constrained by usage.

## Tests to Write

### Higher-Order Function Tests

- `testInferFunctionAsArgument`: define `apply` that takes a function and a value, then call it with a concrete function — types should be consistent
- `testInferFunctionReturnValue`: define a function that returns a function (via leading pipe) — return type should be a function type
- `testInferComposition`: `"f: (| double | increment)"` — f has type matching the composition
- `testInferCurriedApplication`: `"r: (add 1)"` — r has a function type (partially applied)
- `testInferPassFunctionToHigherOrder`: `"r: (map increment [1, 2, 3])"` — r has array type

### Polymorphism Tests

- `testPolymorphicIdentity`: `"id: x"` then `"a: (id 1)"` and `"b: (id \"hi\")"` — both uses succeed with different types
- `testPolymorphicMap`: map used with different function/element types
- `testMonomorphicParameter`: `x` within a function body is monomorphic — can't be used at two different types simultaneously

### Type Annotation Tests

- `testAnnotatedDefinitionMatches`: `"n: *i64 42"` — annotation matches inferred type
- `testAnnotatedDefinitionMismatch`: `"n: *string 42"` — throws type mismatch
- `testAnnotatedFunctionType`: `"f: *(i64|i64) (double x)"` — function type annotation
- `testAliasAnnotation`: `"n: 'i64 42"` — structural alias check

### Nominal vs. Structural Tests

- `testNominalTypeCheckPasses`: value tagged with `*Point` matches `*Point`
- `testNominalTypeCheckFails`: value tagged with `*Point` does not match `*Vec` even if same structure
- `testStructuralTypeCheckPasses`: untagged value matches `'{x: f64, y: f64}` by structure

### Closure Tests

- `testInferClosure`: function that references outer scope name — type includes the captured variable's type correctly
- `testClosurePolymorphism`: captured variable constrains the function's type

### Error Tests

- `testHigherOrderTypeMismatch`: passing a function of wrong type → clear error
- `testPartialApplicationTypeMismatch`: `(add "hello")` where add expects numeric → error

### Compile Tests

- `testCompileHigherOrderFunction`: compiles without error
- `testCompilePolymorphicUsage`: compiles without error

## Success Criteria

1. `swift build` compiles without errors.
2. `swift test` — full suite passes.
3. Functions can be passed as arguments and returned from other functions.
4. Let-polymorphism works correctly — definitions are polymorphic, parameters are monomorphic.
5. Explicit type annotations are checked against inferred types.
6. Curried application infers types correctly through partial application.
7. Nominal and structural type checking behaves correctly per annotation kind.

## Important Notes

- **Don't break existing tests**: All Phase 7 inference tests must still pass.
- **Higher-order functions are already supported by Algorithm W** — the main work here is ensuring all edge cases are handled and writing comprehensive tests.
- **Closures don't need special AST support** — they are regular functions that happen to reference outer names. The type checker already handles this because `x` is looked up in the environment.
- **The value restriction is important**: without it, mutable state (if ever added) could break type safety. Even without mutation, it prevents some confusing type behaviors.
- Keep all types `public` and `Equatable`.
- Run `swift test` before considering this phase complete.
