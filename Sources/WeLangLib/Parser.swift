/// Parse a token stream into an AST `Program`.
public func parse(_ tokens: [Token]) throws -> Program {
    var parser = Parser(tokens: tokens)
    return try parser.parseProgram()
}

// MARK: - Parser Core

struct Parser {
    let tokens: [Token]
    var pos: Int = 0

    /// Peek at the current token without consuming it.
    func peek() -> Token {
        guard pos < tokens.count else {
            let endPos = tokens.last?.span.end ?? 0
            return Token(kind: .eof, span: Span(start: endPos, end: endPos))
        }
        return tokens[pos]
    }

    /// Consume the current token and advance.
    @discardableResult
    mutating func advance() -> Token {
        let token = peek()
        if pos < tokens.count { pos += 1 }
        return token
    }

    /// If the current token matches `kind`, consume and return it; otherwise return nil.
    @discardableResult
    mutating func match(_ kind: TokenKind) -> Token? {
        guard peek().kind == kind else { return nil }
        return advance()
    }

    /// Consume a token of the expected kind, or throw `unexpectedToken`.
    @discardableResult
    mutating func expect(_ kind: TokenKind) throws -> Token {
        if let token = match(kind) { return token }
        throw ParseError.unexpectedToken(span: peek().span)
    }

    /// If the current token is a `.label`, consume it and return `(text, token)`.
    mutating func matchLabel() -> (String, Token)? {
        guard case .label(let text) = peek().kind else { return nil }
        return (text, advance())
    }

    /// Skip any newline tokens.
    mutating func skipNewlines() {
        while case .newline = peek().kind {
            advance()
        }
    }
}

// MARK: - Grammar Rules

extension Parser {

    // MARK: Program

    /// Program = Definition* EOF
    mutating func parseProgram() throws -> Program {
        var definitions: [Definition] = []
        skipNewlines()
        while peek().kind != .eof {
            definitions.append(try parseDefinition())
            skipNewlines()
        }
        return Program(definitions: definitions)
    }

    // MARK: Definition

    /// Definition = Label TypeAnnotation? ":" Expr
    mutating func parseDefinition() throws -> Definition {
        guard let (labelText, labelToken) = matchLabel() else {
            throw ParseError.expectedDefinition(span: peek().span)
        }

        skipNewlines()

        // Disambiguate: colon → no type annotation; label → type annotation.
        let typeAnnotation: Expr?
        if match(.colon) != nil {
            typeAnnotation = nil
        } else if let (typeName, typeToken) = matchLabel() {
            typeAnnotation = .name(typeName, typeToken.span)
            skipNewlines()
            guard match(.colon) != nil else {
                throw ParseError.expectedColon(span: peek().span)
            }
        } else {
            throw ParseError.expectedColon(span: peek().span)
        }

        skipNewlines()

        let value = try parseExpr()
        return Definition(
            label: labelText,
            typeAnnotation: typeAnnotation,
            value: value,
            span: Span(start: labelToken.span.start, end: value.span.end)
        )
    }

    // MARK: Expr

    /// Entry point for a definition value.
    ///
    /// At the definition level, exactly one atom or one grouped expression is
    /// allowed. Application and pipe are only available inside parentheses,
    /// which keeps the grammar unambiguous when multiple definitions appear
    /// on the same line (`foo: 1 bar: 2`).
    ///
    ///     Expr = AtomExpr | GroupExpr
    mutating func parseExpr() throws -> Expr {
        switch peek().kind {
        case .integerLiteral, .floatLiteral, .stringLiteral,
             .interpolatedStringLiteral, .label, .discard:
            return try parseAtomExpr()
        case .leftParen:
            return try parseGroupExpr()
        default:
            throw ParseError.expectedExpression(span: peek().span)
        }
    }

    // MARK: Atom

    /// Parses a single indivisible expression (no application, no pipe).
    ///
    ///     AtomExpr = IntegerLiteral | FloatLiteral | StringLiteral
    ///              | InterpolatedStringLiteral | Label | "_"
    private mutating func parseAtomExpr() throws -> Expr {
        let token = peek()
        switch token.kind {
        case .integerLiteral(let text):
            advance()
            return .integerLiteral(text, token.span)
        case .floatLiteral(let text):
            advance()
            return .floatLiteral(text, token.span)
        case .stringLiteral(let text):
            advance()
            return .stringLiteral(text, token.span)
        case .interpolatedStringLiteral(let text):
            advance()
            return .interpolatedStringLiteral(text, token.span)
        case .label(let text):
            advance()
            return .name(text, token.span)
        case .discard:
            advance()
            return .discard(token.span)
        default:
            throw ParseError.expectedExpression(span: token.span)
        }
    }

    // MARK: Group

    /// Parses a parenthesised expression.
    ///
    ///     GroupExpr = "(" ")"                  → unit
    ///               | "(" PipeExpr ")"         → inner expression (parens transparent)
    private mutating func parseGroupExpr() throws -> Expr {
        let open = advance() // consume '('
        skipNewlines()
        if let close = match(.rightParen) {
            return .unit(Span(start: open.span.start, end: close.span.end))
        }
        let inner = try parsePipeExpr()
        skipNewlines()
        guard match(.rightParen) != nil else {
            throw ParseError.unexpectedToken(span: peek().span)
        }
        return inner
    }

    // MARK: Pipe

    /// Parses a pipe expression (left-associative).
    ///
    ///     PipeExpr = AppExpr ("|" AppExpr)*
    ///
    /// `(A | B | C)` builds `pipe(pipe(A, B), C)`.
    /// Data flows left-to-right: A is evaluated first, its result passes to B,
    /// then to C — the same data-flow order as `(C B A)` written with
    /// right-associative juxtaposition.
    private mutating func parsePipeExpr() throws -> Expr {
        var lhs = try parseAppExpr()
        skipNewlines()
        while match(.pipe) != nil {
            skipNewlines()
            let rhs = try parseAppExpr()
            let span = Span(start: lhs.span.start, end: rhs.span.end)
            lhs = .pipe(lhs, rhs, span)
            skipNewlines()
        }
        return lhs
    }

    // MARK: Application

    /// Parses a right-associative function application sequence.
    ///
    ///     AppExpr = AtomOrGroupExpr+
    ///
    /// `f g h` builds `application(f, application(g, h))`.
    /// Data flows right-to-left mathematically: `h` is innermost.
    private mutating func parseAppExpr() throws -> Expr {
        let first = try parseAtomOrGroupExpr()
        skipNewlines()
        if canStartAtomOrGroup() {
            let rest = try parseAppExpr()
            let span = Span(start: first.span.start, end: rest.span.end)
            return .application(first, rest, span)
        }
        return first
    }

    /// Parses one atom or a nested grouped expression.
    private mutating func parseAtomOrGroupExpr() throws -> Expr {
        if peek().kind == .leftParen {
            return try parseGroupExpr()
        }
        return try parseAtomExpr()
    }

    /// Returns true if the current token can begin an atom or grouped expression.
    private func canStartAtomOrGroup() -> Bool {
        switch peek().kind {
        case .integerLiteral, .floatLiteral, .stringLiteral,
             .interpolatedStringLiteral, .label, .discard, .leftParen:
            return true
        default:
            return false
        }
    }
}
