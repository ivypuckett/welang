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

    /// Expr = IntegerLiteral | FloatLiteral | StringLiteral
    ///      | InterpolatedStringLiteral | Label | "_" | "()"
    mutating func parseExpr() throws -> Expr {
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

        case .leftParen:
            return try parseUnit()

        default:
            throw ParseError.expectedExpression(span: token.span)
        }
    }

    // MARK: Unit

    /// Parses `()` as a unit expression.
    private mutating func parseUnit() throws -> Expr {
        let open = advance() // consume '(' — caller already verified
        skipNewlines()
        guard let close = match(.rightParen) else {
            // Non-empty parens are not supported in this phase.
            throw ParseError.unexpectedToken(span: peek().span)
        }
        return .unit(Span(start: open.span.start, end: close.span.end))
    }
}
