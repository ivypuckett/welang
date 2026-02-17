/// A single lexical token produced by the lexer.
public struct Token: Equatable {
    public let kind: TokenKind
    public let span: Span

    public init(kind: TokenKind, span: Span) {
        self.kind = kind
        self.span = span
    }
}

/// The kind of a lexical token.
///
/// This enum will be extended as the language grows. For now it only
/// contains `eof` so that the scaffolding compiles and tests pass.
public enum TokenKind: Equatable {
    /// End-of-file marker.
    case eof
}

/// Lex the source string into an array of `Token`s.
///
/// The returned token stream always ends with an `eof` token.
public func lex(_ source: String) throws -> [Token] {
    // TODO: implement real tokenization here.
    // For now, just produce eof so downstream stages have something to work with.

    let length = source.utf8.count
    let tokens = [
        Token(
            kind: .eof,
            span: Span(start: length, end: length)
        ),
    ]

    return tokens
}
