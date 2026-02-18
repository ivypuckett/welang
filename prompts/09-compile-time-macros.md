# Phase 9: Compile-Time Macros — Value-Level Metaprogramming

## Goal

Implement welang's compile-time macro system using the `@` sigil. Unlike Lisp's AST-transforming macros or Rust's procedural macros, welang macros are **value-level**: they are ordinary functions that execute at compile time and operate on **values**, not syntax trees. This phase adds:

1. **Macro application syntax**: `@macroName expression`
2. **Compile-time evaluation** of macro functions
3. **Type checking** of macro inputs and outputs
4. **A compile-time evaluation engine** (interpreter) for constant expressions

After this phase:

```we
# @1 replaces the following expression with the number 1
weird: @1 "one"
# Result: weird has value 1, type i64

# @memoize wraps a function with caching
fn: @memoize query

# Macros are just functions — defined normally
double: (multiply x 2)
four: @double 2
# At compile time: double(2) = 4, so four = 4
```

## Background

### Value Macros vs. AST Macros

Most languages with macros operate on syntax trees:
- **Lisp**: macros receive and return S-expressions (code as data, homoiconicity)
- **Rust**: procedural macros receive and return `TokenStream`s
- **Haskell**: Template Haskell splices operate on AST quotations

welang takes a different approach: **macros are regular functions evaluated at compile time**. They receive the **value** of the following expression (or the unevaluated function, if it's a function). This is closer to:
- **Zig's `comptime`**: arbitrary compile-time evaluation
- **C++'s `constexpr`**: compile-time function evaluation
- **Forth's immediate words**: words that execute at compile time

### Macro Semantics

`@f e` means:
1. If `e` is a non-function expression: evaluate `e` at compile time to get a value, then apply `f` to that value at compile time.
2. If `e` is a function (references `x`): pass the unevaluated function `e` to `f` as a value. `f` can wrap, transform, or replace it.
3. The result of `f(e)` replaces the macro application in the AST.
4. The replacement must type-check in the surrounding context (same type inference rules as any other expression).

### Why Value-Level?

Value-level macros are simpler to reason about than AST macros:
- They follow the same type inference rules as regular code
- No need for quasi-quotation or hygiene systems
- The macro author writes normal functions — no special macro API
- Debugging is easier because values are inspectable

The tradeoff is less power: you cannot generate arbitrary syntax. But welang's uniform syntax (everything is S-expressions) makes this less of a limitation.

## Project Context

### Files to Create/Modify

```
Sources/WeLangLib/
    AST.swift            ← add macro application Expr case
    Parser.swift         ← parse @ syntax
    Interpreter.swift    ← NEW: compile-time expression evaluator
    TypeInference.swift  ← handle macro type inference
    Compile.swift        ← add macro expansion pass
    Errors.swift         ← add macro-specific errors
Tests/WeLangTests/
    ParserTests.swift    ← macro parsing tests
    InterpreterTests.swift ← NEW: compile-time evaluation tests
    TypeInferenceTests.swift ← macro type inference tests
    CompileTests.swift   ← end-to-end macro tests
```

### Current Expr Enum (relevant subset)

```swift
public indirect enum Expr: Equatable {
    case integerLiteral(String, Span)
    case floatLiteral(String, Span)
    case stringLiteral(String, Span)
    case name(String, Span)
    case apply(function: Expr, arguments: [Expr], Span)
    case pipe(clauses: [Expr], Span)
    case tuple(entries: [CompoundEntry], Span)
    case array(entries: [CompoundEntry], Span)
    case aliasType(TypeExpr, Span)
    case identifierType(TypeExpr, Span)
    // ...
}
```

## AST Addition

Add a macro application case to `Expr`:

```swift
public indirect enum Expr: Equatable {
    // ... existing cases ...

    /// Compile-time macro application: `@f e`
    /// `macro` is the macro function expression, `argument` is the expression it's applied to.
    case macro(macro: Expr, argument: Expr, Span)
}
```

## Parsing

### Macro Application

When the parser encounters `@` in expression position:

```
MacroExpr = "@" Atom Expr
```

1. Consume `@`
2. Parse the macro name/expression as an atom (usually just a label like `memoize`, but could be any atom)
3. Parse the argument expression

The `@` binds tighter than pipe but looser than postfix access. In practice:

```we
@memoize (query x)     # macro=memoize, argument=(query x)
@double 2              # macro=double, argument=2
@1 "one"               # macro=1, argument="one"
```

Add to `parseAtom()`:

```swift
case .at:
    let start = peek().span.start
    advance()  // consume @
    let macroExpr = try parseAtom()      // the macro function
    let argument = try parseExpr()       // the argument expression
    let span = Span(start: start, end: argument.span.end)
    return .macro(macro: macroExpr, argument: argument, span)
```

**Note**: `@` in the middle of an expression (not at the start of an atom) is a syntax error — macros must appear at the "front" of an expression.

## Compile-Time Interpreter

### Overview

To evaluate macros at compile time, you need a minimal **interpreter** that can evaluate constant expressions. This is a tree-walking interpreter over the AST.

### Value Representation

```swift
/// A compile-time value produced by the interpreter.
public enum Value: Equatable {
    case integer(Int64)
    case unsignedInteger(UInt64)
    case float(Double)
    case string(String)
    case bool(Bool)
    case unit
    case tuple([(String, Value)])
    case array([Value])
    case function(FunctionValue)
}

/// A function value for compile-time evaluation.
public struct FunctionValue: Equatable {
    /// The parameter name (always "x" for welang functions).
    public let param: String
    /// The function body as an AST expression.
    public let body: Expr
    /// Captured environment (closure).
    public let env: [String: Value]
}
```

### Interpreter

```swift
public struct Interpreter {
    var env: [String: Value] = [:]

    /// Evaluate an expression at compile time.
    public mutating func eval(_ expr: Expr) throws -> Value {
        switch expr {
        case .integerLiteral(let text, _):
            guard let n = Int64(text) else { throw MacroError.invalidLiteral(text) }
            return .integer(n)

        case .floatLiteral(let text, _):
            guard let n = Double(text) else { throw MacroError.invalidLiteral(text) }
            return .float(n)

        case .stringLiteral(let text, _):
            return .string(text)

        case .name(let name, let span):
            guard let value = env[name] else {
                throw MacroError.undefinedName(name, span)
            }
            return value

        case .apply(let function, let arguments, _):
            var result = try eval(function)
            for arg in arguments {
                let argVal = try eval(arg)
                result = try applyFunction(result, to: argVal)
            }
            return result

        case .pipe(let clauses, _):
            var current = try eval(clauses[0])
            for clause in clauses.dropFirst() {
                let fn = try eval(clause)
                current = try applyFunction(fn, to: current)
            }
            return current

        case .tuple(let entries, _):
            var fields: [(String, Value)] = []
            for (i, entry) in entries.enumerated() {
                let key: String
                switch entry.key {
                case .implicit: key = String(i)
                case .label(let name, _): key = name
                case .index(let idx, _): key = idx
                case .stringKey(let s, _): key = s
                }
                let val = try eval(entry.value)
                fields.append((key, val))
            }
            return .tuple(fields)

        case .macro(let macroExpr, let argument, _):
            // Recursive: evaluate the macro function, then apply it
            let macroFn = try eval(macroExpr)
            let argVal = try eval(argument)
            return try applyFunction(macroFn, to: argVal)

        case .unit(_):
            return .unit

        case .discard(_):
            return .unit

        default:
            throw MacroError.cannotEvaluateAtCompileTime(expr)
        }
    }

    /// Apply a function value to an argument.
    func applyFunction(_ fn: Value, to arg: Value) throws -> Value {
        switch fn {
        case .function(let funcVal):
            var innerInterp = Interpreter(env: funcVal.env)
            innerInterp.env[funcVal.param] = arg
            return try innerInterp.eval(funcVal.body)

        case .integer(_), .float(_), .string(_), .bool(_):
            // A literal "applied to" something returns itself
            // (welang rule: a number is a function that ignores input and returns itself)
            return fn

        default:
            throw MacroError.notAFunction(fn)
        }
    }
}
```

### Built-in Compile-Time Functions

Seed the interpreter's environment with built-in functions that can be used at compile time:

```swift
func seedCompileTimeBuiltins(_ env: inout [String: Value]) {
    // add, multiply, etc. — implemented as special built-in function values
    // You may want a .builtin(String, (Value) -> Value) case in Value for these
}
```

Alternatively, add a `.builtin(String, (Value) throws -> Value)` case to `Value` for built-in operations that cannot be expressed as welang AST. This is simpler than trying to represent `add` as an AST expression.

```swift
public enum Value: Equatable {
    // ... existing cases ...
    case builtin(String, BuiltinFn)
}

/// Wrapper for built-in function closures (needed for Equatable).
public struct BuiltinFn: Equatable {
    public let name: String
    public let fn: (Value) throws -> Value

    public static func == (lhs: BuiltinFn, rhs: BuiltinFn) -> Bool {
        lhs.name == rhs.name
    }
}
```

## Macro Expansion Pass

Add a new compiler pass that runs **after parsing and before type inference**:

```
source → lex() → parse() → expandMacros() → infer() → generate()
                             ^^^^^^^^^^^^^^
                             NEW PASS
```

### Expansion Algorithm

Walk the AST and replace every `.macro(macro:argument:)` node with the result of compile-time evaluation:

```swift
public func expandMacros(_ program: Program) throws -> Program {
    var expander = MacroExpander()

    // First pass: collect all definitions that are constant (can be evaluated at compile time)
    for def in program.definitions {
        if isCompileTimeEvaluable(def.value) && !referencesX(def.value) {
            let value = try expander.interpreter.eval(def.value)
            expander.interpreter.env[def.label] = value
        }
    }

    // Second pass: expand macros in all definitions
    var expandedDefs: [Definition] = []
    for def in program.definitions {
        let expandedValue = try expander.expandExpr(def.value)
        expandedDefs.append(Definition(
            label: def.label,
            typeAnnotation: def.typeAnnotation,
            value: expandedValue,
            span: def.span
        ))
        // Update the environment if this definition is now constant
        if isCompileTimeEvaluable(expandedValue) && !referencesX(expandedValue) {
            let value = try expander.interpreter.eval(expandedValue)
            expander.interpreter.env[def.label] = value
        }
    }

    return Program(definitions: expandedDefs)
}
```

### Value to Expression Conversion

After macro evaluation, the resulting `Value` must be converted back into an `Expr` node:

```swift
func valueToExpr(_ value: Value, span: Span) -> Expr {
    switch value {
    case .integer(let n): return .integerLiteral(String(n), span)
    case .float(let n): return .floatLiteral(String(n), span)
    case .string(let s): return .stringLiteral(s, span)
    case .bool(let b): return .name(b ? "true" : "false", span)
    case .unit: return .unit(span)
    case .tuple(let fields):
        let entries = fields.map { (k, v) in
            CompoundEntry(key: .label(k, span), value: valueToExpr(v, span: span), span: span)
        }
        return .tuple(entries: entries, span: span)
    case .function(let funcVal):
        return funcVal.body  // Functions stay as their body expression
    default:
        // Fallback
        return .unit(span)
    }
}
```

## Error Type

```swift
public enum MacroError: Error, Equatable, CustomStringConvertible {
    case cannotEvaluateAtCompileTime(Expr)
    case undefinedName(String, Span)
    case invalidLiteral(String)
    case notAFunction(Value)
    case divisionByZero
    // Add more as needed
}
```

Update `CompileError` to include a `.macro(MacroError)` case.

## Type Inference for Macros

After macro expansion, the expanded AST goes through normal type inference. The type of a macro application site is the type of whatever expression the macro produced. No special type rules are needed — macros are fully expanded before type checking.

**However**, during type inference, if a macro application was **not** expanded (because the macro function couldn't be evaluated at compile time), the type checker should report an error:

```swift
case .macro(_, _, let span):
    throw TypeError.unexpandedMacro(span)
```

## Tests to Write

### Parser Tests

- `testParseMacroApplication`: `"r: @double 2"` → `.macro(macro: .name("double"), argument: .integerLiteral("2"))`
- `testParseMacroWithLiteral`: `"r: @1 \"one\""` → `.macro(macro: .integerLiteral("1"), argument: .stringLiteral("one"))`
- `testParseMacroWithSExpr`: `"r: @memoize (query x)"` → macro with s-expression argument
- `testParseMacroChained`: `"r: @a @b 1"` → nested macros

### Interpreter Tests (new file)

- `testEvalIntegerLiteral`: eval `42` → `.integer(42)`
- `testEvalFloatLiteral`: eval `3.14` → `.float(3.14)`
- `testEvalStringLiteral`: eval `"hello"` → `.string("hello")`
- `testEvalUnit`: eval `()` → `.unit`
- `testEvalNameLookup`: env has `x = 42`, eval `x` → `.integer(42)`
- `testEvalApplyBuiltin`: env has `double`, eval `(double 2)` → `.integer(4)`
- `testEvalPipe`: eval `(2 | double)` → `.integer(4)`
- `testEvalLiteralAsFunction`: eval `(3 2 1)` → `.integer(3)` (literal ignores args, returns self)
- `testEvalTuple`: eval `{a: 1, b: 2}` → `.tuple([("a", .integer(1)), ("b", .integer(2))])`
- `testEvalUndefinedName`: throws error

### Macro Expansion Tests

- `testExpandSimpleMacro`: define `double: (multiply x 2)`, then `four: @double 2` → `four` becomes literal `4`
- `testExpandLiteralMacro`: `@1 "anything"` → `1`
- `testExpandChainedMacros`: `@double @double 2` → `8` (double(double(2)))
- `testExpandPreservesNonMacro`: definitions without `@` are unchanged
- `testExpandUndefinedMacro`: `@nonexistent 1` → error

### Compile Tests

- `testCompileMacroExpansion`: `"double: (multiply x 2)\nfour: @double 2"` → compiles
- `testCompileMacroTypeCheck`: macro result is type-checked in context

## Success Criteria

1. `swift build` compiles without errors.
2. `swift test` — full suite passes.
3. `@name expr` syntax parses into `.macro` AST nodes.
4. Compile-time interpreter evaluates constant expressions correctly.
5. Macro expansion replaces `.macro` nodes with evaluated results.
6. The expanded AST passes type inference.
7. Macros compose (chained `@a @b expr` works).

## Important Notes

- **Macros are evaluated top-to-bottom**: a macro can only reference definitions that appear **before** it in the file. This prevents circular dependencies.
- **Not all expressions can be evaluated at compile time**: expressions that reference `x` (runtime input), undefined names, or I/O operations cannot be compile-time evaluated. The expander should report clear errors for these.
- **Functions as macro arguments**: when the argument to `@` is a function (references `x`), the function value (including its body and closure environment) is passed to the macro. The macro can wrap or transform it. This is how `@memoize` would work.
- **Performance**: the compile-time interpreter is a simple tree-walker. It doesn't need to be fast — it runs once during compilation.
- Keep all types `public` and `Equatable`.
- Run `swift test` before considering this phase complete.
