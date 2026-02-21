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

    /// Function application by juxtaposition (right-associative, inside a group).
    /// The first `Expr` is the function, the second is the argument.
    /// `f g h` desugars to `application(f, application(g, h))`.
    /// Data flows right-to-left: the rightmost expression is evaluated first.
    case application(Expr, Expr, Span)

    /// Pipe expression: feeds the output of the left expression into the right.
    /// `(A | B)` means data flows through A then B.
    /// Semantically equivalent to `application(B, A)`, but pipes compose
    /// left-to-right so `(A | B | C)` = `pipe(pipe(A, B), C)`.
    case pipe(Expr, Expr, Span)

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
             .application(_, _, let span),
             .pipe(_, _, let span):
            return span
        }
    }
}
