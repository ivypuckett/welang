/// A complete program: the root of the AST.
public struct Program: Equatable {
    public var definitions: [Definition]

    public init(definitions: [Definition]) {
        self.definitions = definitions
    }
}

/// A top-level definition: binds a label to a typed or untyped expression.
public struct Definition: Equatable {
    public let label: String
    /// Optional type annotation between the label and the colon.
    public let typeAnnotation: Expr?
    public let value: Expr
    /// Covers the full definition from label through value.
    public let span: Span

    public init(label: String, typeAnnotation: Expr?, value: Expr, span: Span) {
        self.label = label
        self.typeAnnotation = typeAnnotation
        self.value = value
        self.span = span
    }
}

/// An expression node in the AST.
///
/// `indirect` allows recursive nesting for future compound expression forms.
public indirect enum Expr: Equatable {
    /// Integer literal: `0`, `42`, `-1`
    case integerLiteral(String, Span)

    /// Floating-point literal: `0.1`, `-3.14`
    case floatLiteral(String, Span)

    /// Standard string literal: `"hello"`
    case stringLiteral(String, Span)

    /// Interpolated string literal: `` `hello {{name}}` ``
    /// Raw content stored; interpolation parsing is Phase 5.
    case interpolatedStringLiteral(String, Span)

    /// Reference to a name (another definition or built-in): `foo`, `add`
    /// The implicit input variable `x` is parsed as `.name("x", span)`.
    case name(String, Span)

    /// Discard / wildcard: `_`
    case discard(Span)

    /// Unit value: `()`
    case unit(Span)

    /// S-expression application: `(f arg1 arg2)`
    /// The function is the first element, arguments follow.
    /// `(add 1 2)` → .apply(function: .name("add"), arguments: [.integerLiteral("1"), .integerLiteral("2")])
    case apply(function: Expr, arguments: [Expr], Span)

    /// Pipe expression: `(a | f | g)`
    /// A chain of clauses where each clause receives the output of the previous.
    /// The `clauses` array has at least 2 elements.
    case pipe(clauses: [Expr], Span)

    /// Lambda with named parameter: `(it: body)`
    /// Renames the implicit `x` to a custom name for clarity in closures.
    /// `(it: do it)` → .lambda(param: "it", body: .apply(.name("do"), [.name("it")]))
    case lambda(param: String, body: Expr, Span)

    /// The source span for this expression node.
    public var span: Span {
        switch self {
        case .integerLiteral(_, let span),
             .floatLiteral(_, let span),
             .stringLiteral(_, let span),
             .interpolatedStringLiteral(_, let span),
             .name(_, let span),
             .discard(let span),
             .unit(let span),
             .apply(_, _, let span),
             .pipe(_, let span),
             .lambda(_, _, let span):
            return span
        }
    }
}
