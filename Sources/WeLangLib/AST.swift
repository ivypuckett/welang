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

/// A single key-value entry in a compound literal.
public struct CompoundEntry: Equatable {
    public let key: CompoundKey
    public let value: Expr
    public let span: Span

    public init(key: CompoundKey, value: Expr, span: Span) {
        self.key = key
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

    /// Tuple/object literal: `{1, 0.1}`, `{label: 1}`, `{"key": "value"}`
    case tuple(entries: [CompoundEntry], Span)

    /// Array/map literal: `[12, 24]`, `[key: 12]`, `["k": 1]`
    case array(entries: [CompoundEntry], Span)

    /// Dot access on a tuple/object by label: `x.label`
    case dotAccess(expr: Expr, field: String, Span)

    /// Bracket index access: `x[0]`, `x["key"]`
    case bracketAccess(expr: Expr, index: Expr, Span)

    /// Computed dot-bracket access: `x.[ expr ]`
    case computedAccess(expr: Expr, index: Expr, Span)

    /// The source span for this expression node.
    public var span: Span {
        switch self {
        case .integerLiteral(_, let span),
             .floatLiteral(_, let span),
             .stringLiteral(_, let span),
             .interpolatedStringLiteral(_, let span),
             .name(_, let span),
             .discard(let span),
             .unit(let span):
            return span
        case .apply(_, _, let span),
             .pipe(_, let span),
             .lambda(_, _, let span),
             .tuple(_, let span),
             .array(_, let span),
             .dotAccess(_, _, let span),
             .bracketAccess(_, _, let span),
             .computedAccess(_, _, let span):
            return span
        }
    }
}
