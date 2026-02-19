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
}
