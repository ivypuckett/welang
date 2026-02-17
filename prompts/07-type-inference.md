# Phase 7: Type Inference — Algorithm W and the Hindley-Milner Type System

## Goal

Implement type inference for welang using **Algorithm W** (Damas-Hindley-Milner), the standard algorithm for ML-family type inference. This phase introduces a new compiler pass between parsing and code generation that:

1. **Infers types** for all expressions without requiring explicit annotations
2. **Checks explicit type annotations** against inferred types
3. **Implements unification** for type constraint solving
4. **Implements let-polymorphism** (prenex / rank-1 polymorphism)
5. **Distinguishes structural (alias) from nominal (identifier) types**

After this phase, the compiler can detect type errors:

```we
# OK: inferred as u32 → u32 → u32
add: (x | y | addIntegers x y)

# OK: inferred return type from body
double: (multiply x 2)

# ERROR: type mismatch — string where u32 expected
bad: (add "hello" 1)
```

## Background

### Algorithm W (Damas-Milner, 1982)

Algorithm W is the canonical type inference algorithm for the Hindley-Milner type system, used in ML, Haskell (with extensions), OCaml, and F#. It works by:

1. **Generating fresh type variables** for unknown types
2. **Collecting constraints** as expressions are traversed
3. **Solving constraints via unification** — finding a most general substitution that satisfies all constraints
4. **Generalizing** let-bound definitions to produce polymorphic type schemes

The key insight of HM: type inference is **decidable** and produces **principal types** (the most general type for any expression) without any annotations.

### Key Concepts

- **Monotype (τ)**: A concrete type or type variable. Examples: `u32`, `α → β`, `{x: f64, y: f64}`.
- **Polytype / Type Scheme (σ)**: A universally quantified type: `∀α. α → α`. This is what let-bound definitions get.
- **Type Variable**: A placeholder for an unknown type, written as `α`, `β`, `γ` (or internally as `T0`, `T1`, etc.).
- **Substitution**: A mapping from type variables to types. Applying a substitution replaces variables with their bound types.
- **Unification**: Given two types, find the most general substitution that makes them equal. For example, unifying `α → u32` with `string → β` yields `{α ↦ string, β ↦ u32}`.
- **Generalization**: Converting a monotype to a polytype by universally quantifying over free type variables not in the environment.
- **Instantiation**: Converting a polytype back to a monotype by replacing quantified variables with fresh type variables.

### welang-Specific Extensions

1. **Implicit parameter `x`**: Every function body has access to `x`, the implicit single argument. `x` gets a fresh type variable when type-checking a function definition.

2. **Structural vs. Nominal type checking**:
   - Alias types (`'T`): checked structurally — if the structure matches, the type is satisfied.
   - Identifier types (`*T`): checked nominally — the value must have been explicitly constructed with that type tag.
   - This is an extension to standard HM that adds a nominal/structural distinction during unification.

3. **Curried functions**: `(add 1 2)` is implicitly `(add 1) 2`. During inference, `add` gets type `α → β → γ`, and each application narrows the types.

4. **Pipe desugaring for inference**: `(a | f)` is semantically `f(a)`. The type checker treats pipes as chained function applications.

## Project Context

### Files to Create/Modify

```
Sources/WeLangLib/
    TypeInference.swift   ← NEW: Algorithm W implementation
    Types.swift           ← NEW: internal type representation for inference
    AST.swift             ← add type annotation attachment to Expr
    Compile.swift         ← add type inference pass to pipeline
    Errors.swift          ← add TypeError
Tests/WeLangTests/
    TypeInferenceTests.swift ← NEW: comprehensive type inference tests
```

### Compilation Pipeline Update

```
source → lex() → parse() → infer() → generate() → LLVM IR
                            ^^^^^^^
                            NEW PASS
```

Update `Compile.swift`:

```swift
public func compile(_ source: String) throws {
    let tokens = try lex(source)
    let ast = try parse(tokens)
    let typedAst = try infer(ast)   // NEW
    try generate(typedAst)
}
```

## Internal Type Representation

Define types used by the inference engine (separate from the `TypeExpr` AST nodes, which represent what the user wrote):

```swift
/// Internal type representation for the type inference engine.
public indirect enum Type: Equatable, CustomStringConvertible {
    /// Type variable (unknown, to be solved): α, β, ...
    case variable(Int)  // unique ID

    /// Primitive type
    case primitive(PrimitiveType)

    /// Function type: input → output
    case function(Type, Type)

    /// Tuple/record type
    case tuple([(String, Type)])

    /// Array type: [keyType: valueType]
    case array(key: Type, value: Type)

    /// Unit type
    case unit

    /// Nominal wrapper: a structural type tagged with a name.
    /// Two nominal types are equal only if names match.
    case nominal(name: String, inner: Type)
}

public enum PrimitiveType: String, Equatable {
    case u8, u16, u32, u64
    case i8, i16, i32, i64
    case f32, f64
    case string
    case bool
}
```

### Type Scheme

```swift
/// A polymorphic type scheme: ∀ vars. type
public struct TypeScheme: Equatable {
    /// Universally quantified type variable IDs.
    public let quantified: Set<Int>

    /// The underlying monotype.
    public let type: Type

    public init(quantified: Set<Int>, type: Type) { ... }

    /// A monomorphic scheme (no quantified variables).
    public static func mono(_ type: Type) -> TypeScheme {
        TypeScheme(quantified: [], type: type)
    }
}
```

## Algorithm W Implementation

### Type Environment

```swift
/// Maps definition names to their type schemes.
struct TypeEnv {
    var bindings: [String: TypeScheme] = [:]

    func lookup(_ name: String) -> TypeScheme? { bindings[name] }

    func extending(_ name: String, with scheme: TypeScheme) -> TypeEnv {
        var env = self
        env.bindings[name] = scheme
        return env
    }

    /// Free type variables in the environment.
    func freeVars() -> Set<Int> { ... }
}
```

### Substitution

```swift
/// A type substitution: mapping from type variable IDs to types.
struct Substitution {
    var map: [Int: Type] = [:]

    /// Apply this substitution to a type.
    func apply(_ type: Type) -> Type { ... }

    /// Apply this substitution to a type scheme.
    func apply(_ scheme: TypeScheme) -> TypeScheme { ... }

    /// Apply this substitution to an environment.
    func apply(_ env: TypeEnv) -> TypeEnv { ... }

    /// Compose two substitutions: (s2 ∘ s1)
    func compose(with other: Substitution) -> Substitution { ... }
}
```

### Fresh Variable Generator

```swift
struct TypeVarGenerator {
    var nextId: Int = 0

    mutating func fresh() -> Type {
        let id = nextId
        nextId += 1
        return .variable(id)
    }
}
```

### Unification

The **occurs check** prevents infinite types (e.g., `α = α → β`):

```swift
/// Unify two types, producing a substitution that makes them equal.
func unify(_ t1: Type, _ t2: Type) throws -> Substitution {
    switch (t1, t2) {
    case (.variable(let id), let t):
        if t == .variable(id) { return Substitution() }  // same var
        if occurs(id, in: t) { throw TypeError.infiniteType(id, t) }
        return Substitution(map: [id: t])

    case (let t, .variable(let id)):
        return try unify(.variable(id), t)

    case (.primitive(let a), .primitive(let b)):
        if a == b { return Substitution() }
        throw TypeError.typeMismatch(t1, t2)

    case (.function(let in1, let out1), .function(let in2, let out2)):
        let s1 = try unify(in1, in2)
        let s2 = try unify(s1.apply(out1), s1.apply(out2))
        return s1.compose(with: s2)

    case (.tuple(let fields1), .tuple(let fields2)):
        // Unify field by field (structural)
        ...

    case (.array(let k1, let v1), .array(let k2, let v2)):
        let s1 = try unify(k1, k2)
        let s2 = try unify(s1.apply(v1), s1.apply(v2))
        return s1.compose(with: s2)

    case (.unit, .unit):
        return Substitution()

    case (.nominal(let n1, let inner1), .nominal(let n2, let inner2)):
        if n1 != n2 { throw TypeError.nominalMismatch(n1, n2) }
        return try unify(inner1, inner2)

    default:
        throw TypeError.typeMismatch(t1, t2)
    }
}

/// Occurs check: does variable `id` appear in `type`?
func occurs(_ id: Int, in type: Type) -> Bool { ... }
```

### Generalization and Instantiation

```swift
/// Generalize a type to a type scheme by quantifying variables not free in the environment.
func generalize(env: TypeEnv, type: Type) -> TypeScheme {
    let envFree = env.freeVars()
    let typeFree = freeVars(type)
    let quantified = typeFree.subtracting(envFree)
    return TypeScheme(quantified: quantified, type: type)
}

/// Instantiate a type scheme by replacing quantified variables with fresh ones.
func instantiate(_ scheme: TypeScheme, gen: inout TypeVarGenerator) -> Type {
    var sub = Substitution()
    for v in scheme.quantified {
        sub.map[v] = gen.fresh()
    }
    return sub.apply(scheme.type)
}
```

### The W Function

The core of Algorithm W. Infer the type of an expression in a given environment:

```swift
/// Infer the type of an expression. Returns the substitution and the inferred type.
func w(env: TypeEnv, expr: Expr, gen: inout TypeVarGenerator) throws -> (Substitution, Type) {
    switch expr {
    case .integerLiteral(let text, _):
        // Default to i64; if text starts with non-negative and fits, could be u64
        // For now, assign a fresh variable constrained to numeric types
        // Simplification: default integer literals to i64
        return (Substitution(), .primitive(.i64))

    case .floatLiteral(_, _):
        return (Substitution(), .primitive(.f64))

    case .stringLiteral(_, _):
        return (Substitution(), .primitive(.string))

    case .name(let name, let span):
        guard let scheme = env.lookup(name) else {
            throw TypeError.undefinedName(name, span)
        }
        let t = instantiate(scheme, gen: &gen)
        return (Substitution(), t)

    case .apply(let function, let arguments, _):
        // Curried application: ((f arg1) arg2) ...
        var (s, funcType) = try w(env: env, expr: function, gen: &gen)
        for arg in arguments {
            let (s2, argType) = try w(env: s.apply(env), expr: arg, gen: &gen)
            let resultType = gen.fresh()
            let s3 = try unify(
                s2.apply(s.apply(funcType)),
                .function(argType, resultType)
            )
            s = s.compose(with: s2).compose(with: s3)
            funcType = s3.apply(resultType)
        }
        return (s, funcType)

    case .pipe(let clauses, _):
        // Desugar: (a | f | g) = g(f(a))
        var (s, currentType) = try w(env: env, expr: clauses[0], gen: &gen)
        for clause in clauses.dropFirst() {
            let (s2, clauseType) = try w(env: s.apply(env), expr: clause, gen: &gen)
            let resultType = gen.fresh()
            let s3 = try unify(
                s2.apply(clauseType),
                .function(s2.apply(s.apply(currentType)), resultType)
            )
            s = s.compose(with: s2).compose(with: s3)
            currentType = s3.apply(resultType)
        }
        return (s, currentType)

    // ... handle other cases: tuple, array, discard, unit, type annotations ...
    }
}
```

### Inferring Definitions

For each top-level definition:

```swift
func inferDefinition(env: inout TypeEnv, def: Definition, gen: inout TypeVarGenerator) throws {
    // The value of a definition is implicitly a function from x → body
    // Introduce a fresh type variable for x
    let xType = gen.fresh()
    let bodyEnv = env.extending("x", with: .mono(xType))

    let (s, bodyType) = try w(env: bodyEnv, expr: def.value, gen: &gen)

    // If the body references x, the definition is a function x → body
    // If x is unused, the definition is just the body value
    let defType: Type
    if referencesX(def.value) {
        defType = .function(s.apply(xType), bodyType)
    } else {
        defType = bodyType
    }

    // Generalize and add to environment (let-polymorphism)
    let scheme = generalize(env: s.apply(env), type: s.apply(defType))
    env = s.apply(env).extending(def.label, with: scheme)
}
```

`referencesX` checks if the expression contains `.name("x", _)` anywhere.

### Top-Level Inference

```swift
/// Type-check a program. Returns the program unchanged (for now) or throws TypeError.
public func infer(_ program: Program) throws -> Program {
    var env = TypeEnv()
    var gen = TypeVarGenerator()

    // Seed environment with built-in functions
    seedBuiltins(&env, gen: &gen)

    for def in program.definitions {
        try inferDefinition(env: &env, def: def, gen: &gen)
    }

    return program  // In a future phase, we'll attach inferred types to the AST
}
```

### Built-in Function Types

Seed the environment with types for built-in operations:

```swift
func seedBuiltins(_ env: inout TypeEnv, gen: inout TypeVarGenerator) {
    // Arithmetic: ∀a. a → a → a (where a is numeric)
    // Simplified for now: specific overloads or generic numeric
    let a = gen.fresh()
    env.bindings["add"] = TypeScheme(
        quantified: [/* a's id */],
        type: .function(a, .function(a, a))
    )
    // ... similarly for multiply, subtract, etc.

    // Comparison: ∀a. a → a → bool
    // toString: ∀a. a → string
    // etc.
}
```

The exact set of built-ins will grow — for this phase, include enough to write meaningful tests (at minimum: `add`, `multiply`, `negate`, `toString`, `greaterThan`).

## Error Type

```swift
public enum TypeError: Error, Equatable, CustomStringConvertible {
    case typeMismatch(Type, Type)
    case undefinedName(String, Span)
    case infiniteType(Int, Type)
    case nominalMismatch(String, String)
    // Add more as needed
}
```

Update `CompileError` to include a `.type(TypeError)` case.

## Tests to Write

Create `Tests/WeLangTests/TypeInferenceTests.swift`.

### Unification Tests
- `testUnifySameVariable`: unifying `α` with `α` → empty substitution
- `testUnifyVariableWithPrimitive`: `α` with `u32` → `{α ↦ u32}`
- `testUnifyPrimitives`: `u32` with `u32` → empty substitution
- `testUnifyPrimitiveMismatch`: `u32` with `string` → throws `typeMismatch`
- `testUnifyFunctions`: `(α → β)` with `(u32 → string)` → `{α ↦ u32, β ↦ string}`
- `testUnifyOccursCheck`: `α` with `(α → u32)` → throws `infiniteType`
- `testUnifyNominalSameName`: `*Point{...}` with `*Point{...}` → structural unification of inner
- `testUnifyNominalDifferentName`: `*Point` with `*Vec` → throws `nominalMismatch`

### Inference Tests
- `testInferIntegerLiteral`: `"x: 42"` → x has integer type
- `testInferFloatLiteral`: `"x: 3.14"` → x has f64
- `testInferStringLiteral`: `"x: \"hello\""` → x has string
- `testInferUnitLiteral`: `"x: ()"` → x has unit
- `testInferNameReference`: `"a: 1\nb: a"` → b has same type as a
- `testInferFunctionApplication`: `"r: (add 1 2)"` → r has numeric type
- `testInferPipeExpression`: `"r: (1 | add 2)"` → r has numeric type
- `testInferPolymorphicDefinition`: a definition used at two different types
- `testInferUndefinedName`: `"r: (foo 1)"` where foo is not defined → throws `undefinedName`
- `testInferTypeMismatch`: `"r: (add \"hello\" 1)"` → throws `typeMismatch`

### Let-Polymorphism Tests
- `testLetPolymorphism`: define `id: x` (identity), then use it as both `(id 1)` and `(id "hello")` — both should succeed because `id` has type `∀α. α → α`

### Compile Pipeline Tests
- `testCompileWithTypeCheck`: `"x: 42"` passes the full pipeline
- `testCompileTypeMismatch`: produces a `CompileError.type(...)` error

## Success Criteria

1. `swift build` compiles without errors.
2. `swift test` — full suite passes.
3. Algorithm W correctly infers types for all expression forms.
4. Unification handles all type constructors (primitives, functions, tuples, arrays, unit, nominal).
5. Let-polymorphism works: definitions are generalized and can be used at different types.
6. Type errors produce clear `TypeError` messages.
7. The compilation pipeline includes the new `infer()` pass.

## Important Notes

- **Algorithm W is well-studied**: Follow the Damas-Milner paper (1982) or any standard PL textbook (e.g., Pierce's *Types and Programming Languages*, Chapter 22). The algorithm is deterministic and produces principal types.
- **Substitution composition order matters**: `s1.compose(with: s2)` should apply `s2` to all types in `s1`, then merge. This is standard "right-biased" composition.
- **The occurs check is essential**: Without it, the algorithm can loop forever on pathological inputs.
- **Start simple**: Get basic literals and function application working first. Add tuple/array type inference incrementally.
- **Built-ins are temporary**: The set of built-in functions will eventually come from the standard library (Phase 10). For now, hard-code a minimal set for testing.
- Keep all public types `public` and `Equatable`.
- Run `swift test` before considering this phase complete.
