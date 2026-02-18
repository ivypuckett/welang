# Phase 3: S-Expressions and Pipe Combinators

## Goal

Implement parsing for welang's two forms of function application:

1. **Prefix notation** (S-expressions): Lisp-style `(f arg1 arg2)` where the first element is applied to the rest.
2. **Postfix notation** (pipe combinators): Forth-style `(a | f | g)` where data flows left-to-right through a pipeline.

These are welang's only mechanism for computation. There are no infix operators.

3. **Lambda with named parameter**: `(name: body)` where the implicit parameter `x` is renamed for clarity, especially useful in nested closures.

After this phase, the parser can handle programs like:

```we
result: (add 1 2)
piped: (1 | increment | double)
mixed: (1 | add 2 | multiply 3)
explicit: (1 | 3 2 | 6 5 4)
nested: (add (multiply 2 3) 4)
identity: (x)

# Lambda with named parameter — renames x to "it" for clarity
nested: (something (it: do it) x)

# Named parameter with pipes
transform: (it: it | double | increment)
```

## Background

### S-Expression Semantics (Lisp Heritage)

In welang, parenthesized expressions are S-expressions following Lisp conventions. Within a **clause** (a group of tokens not separated by `|`), the first element is the function and subsequent elements are arguments:

```we
(add 1 2)        # apply `add` to arguments 1, 2
(3 2 1)          # apply 3 to 2, then to 1 — since 3 is a number
                 # (a function from nothing to itself), the result is 3
```

Because welang functions are **always monadic** (single-argument, like ML's curried functions), `(add 1 2)` is syntactic sugar for `((add 1) 2)` — `add` applied to `1` returns a function, which is then applied to `2`. This is standard **curried application** from the ML tradition.

### Pipe Semantics (Forth Heritage)

The pipe `|` is a **clause combinator** that threads output from left to right, inspired by Forth's stack-based composition and Unix pipes:

```we
(1 | increment | double)
# Equivalent to: double(increment(1))
```

Each clause between pipes is an S-expression. The result of the left clause becomes the **implicit input** (`x`) of the right clause:

```we
(1 | add 2)
# The clause `add 2` has implicit input from the left.
# Equivalent to: add(2)(1) — but semantically, 1 becomes x in `add 2 x`
```

### Mixed Prefix and Postfix

Pipes separate an expression into **clauses**. Within each clause, prefix (S-expression) application rules apply:

```we
(1 | 3 2 | 6 5 4 | 10 9 8 7)
# Clause 1: literal 1
# Clause 2: apply 3 to [2], with implicit input from clause 1
# Clause 3: apply 6 to [5, 4], with implicit input from clause 2
# Clause 4: apply 10 to [9, 8, 7], with implicit input from clause 3
```

### Leading Pipe

A clause may begin with a pipe, which means the implicit input `x` of the enclosing function is threaded in:

```we
uniform: (
  | add [x, 1]
  | increment
)
# `| add [x, 1]` — leading pipe, so x is threaded from the function's argument
```

### Lambda with Named Parameter (Closure Clarity)

By default, every function's input is the implicit variable `x`. This works well for simple functions, but in nested closures it becomes ambiguous which `x` you mean. welang allows **renaming** the parameter using the syntax `(name: body)`:

```we
nested: (something (it: do it) x)
```

Here, `(it: do it)` is a lambda (anonymous function) where:
- `it` is the parameter name (replaces `x` inside this lambda)
- `do it` is the body — applies `do` to `it`
- Within the body, `it` refers to this lambda's input, while `x` would refer to the outer function's input

This is directly analogous to ML's `fn it => do(it)` or Haskell's `\it -> do it`. The `name:` prefix at the start of a parenthesized expression triggers lambda parsing.

Named parameters also work with pipes:

```we
transform: (it: it | double | increment)
# Equivalent to: fn it => increment(double(it))
```

## Project Context

### Files to Modify

```
Sources/WeLangLib/
    AST.swift        ← add Expr cases for application and pipe
    Parser.swift     ← extend expression parsing
    Errors.swift     ← add any needed parse errors
Tests/WeLangTests/
    ASTTests.swift   ← tests for new AST nodes
    ParserTests.swift ← comprehensive tests
```

### Current Expr Enum (from Phase 2)

```swift
public indirect enum Expr: Equatable {
    case integerLiteral(String, Span)
    case floatLiteral(String, Span)
    case stringLiteral(String, Span)
    case interpolatedStringLiteral(String, Span)
    case name(String, Span)
    case discard(Span)
    case unit(Span)
}
```

## AST Additions

Add these cases to `Expr`:

```swift
public indirect enum Expr: Equatable {
    // ... existing cases from Phase 2 ...

    /// S-expression application: `(f arg1 arg2)`
    /// The function is the first element, arguments follow.
    /// `(add 1 2)` → .apply(func: .name("add"), args: [.integerLiteral("1"), .integerLiteral("2")])
    case apply(function: Expr, arguments: [Expr], Span)

    /// Pipe expression: `(a | f | g)`
    /// A chain of clauses where each clause receives the output of the previous.
    /// The `clauses` array has at least 2 elements.
    case pipe(clauses: [Expr], Span)

    /// Lambda with named parameter: `(it: body)`
    /// Renames the implicit `x` to a custom name for clarity in closures.
    /// `(it: do it)` → .lambda(param: "it", body: .apply(.name("do"), [.name("it")]))
    case lambda(param: String, body: Expr, Span)
}
```

### Representation Strategy

**Clauses**: Each clause in a pipe is itself an expression. A multi-token clause like `add 2` becomes `.apply(function: .name("add"), arguments: [.integerLiteral("2")])`. A single-token clause like `increment` is just `.name("increment")`.

**Pipes**: `(1 | add 2 | multiply 3)` becomes:
```
.pipe(clauses: [
    .integerLiteral("1", _),
    .apply(function: .name("add"), arguments: [.integerLiteral("2", _)], _),
    .apply(function: .name("multiply"), arguments: [.integerLiteral("3", _)], _)
], _)
```

**Nested S-expressions**: `(add (multiply 2 3) 4)` becomes:
```
.apply(
    function: .name("add"),
    arguments: [
        .apply(function: .name("multiply"), arguments: [.integerLiteral("2"), .integerLiteral("3")]),
        .integerLiteral("4")
    ]
)
```

**Single-element parens**: `(x)` is just the inner expression — no wrapping apply. It's `.name("x")`.

**Leading pipe**: `(| add 1 | increment)` — the parser should generate a `.pipe` whose first clause is the implicit input. Represent this as a pipe where the first clause is a special sentinel. Use `.name("x", span)` as the implicit first clause, since the leading pipe means "take the function's implicit input."

**Lambda with named parameter**: `(it: do it)` becomes:
```
.lambda(
    param: "it",
    body: .apply(function: .name("do"), arguments: [.name("it")]),
    _
)
```

The body of a lambda is parsed using the same `PipeExpr` rule — it can contain pipes:

```
(it: it | double | increment)
→ .lambda(param: "it", body: .pipe([.name("it"), .name("double"), .name("increment")]), _)
```

## Parsing Rules

### Expression Parsing (Updated)

Extend the expression parser to handle parenthesized expressions:

```
Expr = Atom
     | "(" PipeExpr ")"

PipeExpr = Clause ("|" Clause)*

Clause = Atom+      # one or more atoms; first is function, rest are arguments

Atom = IntegerLiteral
     | FloatLiteral
     | StringLiteral
     | InterpolatedStringLiteral
     | Label → Expr.name
     | "_" → Expr.discard
     | "(" PipeExpr ")"     # nested parens (recursion)
```

### Detailed Parsing Logic

1. **`parseExpr()`**: Entry point for expression parsing.
   - If current token is `.leftParen`: call `parseParen()`
   - Otherwise: call `parseAtom()`

2. **`parseParen()`**: Parse a parenthesized expression.
   - Consume `(`
   - If immediately followed by `)`: return `.unit`
   - Handle **leading pipe**: if current token is `|`, insert `.name("x", span)` as the first clause and proceed to parse the rest as a pipe expression
   - Handle **lambda with named parameter**: if current token is `.label` **and** the next token is `.colon`, this is a lambda. Consume the label (parameter name), consume `:`, parse the body as a `PipeExpr`, consume `)`, return `.lambda(param:body:span:)`
   - Otherwise, parse a `PipeExpr`
   - Consume `)`

   **Lambda disambiguation**: The `(label: ...)` form is unambiguous inside parentheses. In welang, `label:` inside `()` is always a lambda parameter. Compare with `{label: ...}` which is a tuple entry and `[label: ...]` which is an array entry — each bracket type has its own meaning for `label:`.

3. **`parsePipeExpr()`**: Parse a sequence of pipe-separated clauses.
   - Parse the first clause
   - If no `|` follows, unwrap: if the clause has a single element, return it directly; if it has multiple elements, return `.apply`
   - If `|` follows, continue parsing clauses and return `.pipe`

4. **`parseClause()`**: Parse a sequence of atoms within a single clause.
   - Collect all atoms until `)` or `|` is seen
   - If one atom: return it directly
   - If multiple atoms: return `.apply(function: first, arguments: rest)`

5. **`parseAtom()`**: Parse a single atomic expression.
   - Integer, float, string, interpolated string, label, discard, or nested paren

### Newline Handling Inside Parentheses

Inside parenthesized expressions, **newlines are insignificant** — they are treated as whitespace. This allows multi-line expressions:

```we
result: (
  add
  1
  2
)
```

The parser should skip `.newline` tokens when inside parentheses. Track paren depth or use a flag to determine when newlines should be skipped.

### Error Cases

Add to `ParseError` if not already present:

- `expectedClosingParen(span: Span)`: when `)` is missing
- `emptyClause(span: Span)`: when a pipe `|` is followed by another `|` or `)` with no content between

## Tests to Write

### AST Tests

- `testApplyEquality`: two `.apply` with same function and arguments are equal
- `testApplyInequality`: different function or arguments → not equal
- `testPipeEquality`: two `.pipe` with same clauses are equal
- `testPipeInequality`: different clauses → not equal
- `testLambdaEquality`: two `.lambda` with same param and body are equal
- `testLambdaInequality`: different param names → not equal
- `testLambdaBodyInequality`: same param, different body → not equal

### Parser Tests

**Basic S-expressions:**
- `testParseSingleElementParen`: `"id: (x)"` → definition with `.name("x")`
- `testParseUnitExpr`: `"u: ()"` → definition with `.unit`
- `testParseApplyOneArg`: `"r: (increment 1)"` → `.apply(.name("increment"), [.integerLiteral("1")])`
- `testParseApplyTwoArgs`: `"r: (add 1 2)"` → `.apply(.name("add"), [.integerLiteral("1"), .integerLiteral("2")])`

**Nested S-expressions:**
- `testParseNestedApply`: `"r: (add (multiply 2 3) 4)"` → nested `.apply`
- `testParseDeeplyNested`: `"r: (f (g (h 1)))"` → three levels of nesting

**Pipe expressions:**
- `testParsePipeTwoClauses`: `"r: (1 | increment)"` → `.pipe([.integerLiteral("1"), .name("increment")])`
- `testParsePipeThreeClauses`: `"r: (1 | add 2 | multiply 3)"` → three-element pipe
- `testParsePipeSingleTokenClauses`: `"r: (1 | 2 | 3)"` → pipe of three literals

**Leading pipe:**
- `testParseLeadingPipe`: `"f: (| increment)"` → `.pipe([.name("x"), .name("increment")])`
- `testParseLeadingPipeMultiple`: `"f: (| add 1 | double)"` → pipe with implicit x first

**Multi-line expressions:**
- `testParseMultilineSExpr`: definition with value spanning multiple lines:
  ```
  r: (
    add
    1
    2
  )
  ```
  → same as `r: (add 1 2)`

- `testParseMultilinePipe`: definition with multi-line piped expression:
  ```
  r: (
    1
    | add 2
    | multiply 3
  )
  ```

**Mixed:**
- `testParseMixedPrefixPostfix`: `"r: (1 | 3 2 | 6 5 4)"` → correct pipe with apply clauses
- `testParseNumberAsFunction`: `"r: (3 2 1)"` → `.apply(.integerLiteral("3"), [.integerLiteral("2"), .integerLiteral("1")])`

**Lambda with named parameter:**
- `testParseLambdaSimple`: `"f: (it: it)"` → `.lambda(param: "it", body: .name("it"))`
- `testParseLambdaWithApply`: `"f: (it: do it)"` → `.lambda(param: "it", body: .apply(.name("do"), [.name("it")]))`
- `testParseLambdaWithPipe`: `"f: (it: it | double | increment)"` → lambda whose body is a pipe
- `testParseLambdaAsArgument`: `"r: (something (it: do it) x)"` → lambda nested as argument in apply
- `testParseLambdaNestedInPipe`: `"r: (data | (item: transform item))"` → lambda as a pipe clause
- `testParseLambdaDifferentName`: `"f: (val: process val)"` → any label can be a param name

**Error cases:**
- `testParseMissingClosingParen`: `"r: (add 1"` → throws `ParseError.expectedClosingParen`
- `testParseEmptyClause`: `"r: (1 | | 2)"` → throws `ParseError.emptyClause`

### Compile Tests

- `testCompileSExpr`: `"r: (add 1 2)"` → compiles without error (codegen is a no-op for now)
- `testCompilePipeExpr`: `"r: (1 | 2 | 3)"` → compiles without error

## Success Criteria

1. `swift build` compiles without errors.
2. `swift test` — full suite passes.
3. S-expressions with prefix notation parse correctly into `.apply` nodes.
4. Pipe expressions parse correctly into `.pipe` nodes.
5. Nested expressions work to arbitrary depth.
6. Multi-line expressions (newlines inside parens) parse correctly.
7. Leading pipe inserts the implicit `x` as the first clause.
8. Lambda with named parameter `(name: body)` parses into `.lambda` nodes.

## Important Notes

- **Currying is implicit**: `(add 1 2)` is represented as a single `.apply` with two arguments. The semantic phase (later) will desugar it into nested single-argument applications. The parser does not need to curry.
- **Newlines inside parens are whitespace**: this is critical for readability. Track paren nesting depth to control newline significance.
- **No operators**: there is no `+`, `-`, etc. All computation is function application. The minus sign in `-1` is part of the number literal, not an operator.
- **Lambda is syntactic sugar**: `(it: expr)` is equivalent to a function definition where the parameter is named `it` instead of `x`. At the type level, it is `∀α β. α → β` just like any other function. The difference is purely about scope naming.
- **`label:` disambiguation by bracket type**: `(label: ...)` is a lambda. `{label: ...}` is a tuple entry. `[label: ...]` is an array entry. Each bracket context gives `:` its own meaning.
- Run `swift test` before considering this phase complete.
