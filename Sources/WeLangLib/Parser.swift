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
        if pos < tokens.count {
            return tokens[pos]
        }
        let endPos = tokens.last?.span.end ?? 0
        return Token(kind: .eof, span: Span(start: endPos, end: endPos))
    }

    /// Consume the current token and advance.
    @discardableResult
    mutating func advance() -> Token {
        let token = peek()
        if pos < tokens.count {
            pos += 1
        }
        return token
    }

    /// Consume a token of the expected kind, or throw `unexpectedToken`.
    @discardableResult
    mutating func expect(_ kind: TokenKind) throws -> Token {
        let token = peek()
        guard token.kind == kind else {
            throw ParseError.unexpectedToken(span: token.span)
        }
        return advance()
    }

    /// Check if the current token matches a kind (by value equality).
    func check(_ kind: TokenKind) -> Bool {
        return peek().kind == kind
    }

    /// Skip any newline tokens.
    mutating func skipNewlines() {
        while case .newline = peek().kind {
            advance()
        }
    }

    /// Returns the label text if the current token is `.label`, else nil.
    func checkLabel() -> String? {
        if case .label(let text) = peek().kind {
            return text
        }
        return nil
    }
}

// MARK: - Grammar Rules

extension Parser {

    // MARK: Program

    mutating func parseProgram() throws -> Program {
        var definitions: [Definition] = []

        while true {
            skipNewlines()
            if check(.eof) { break }
            let def = try parseDefinition()
            definitions.append(def)
        }

        return Program(definitions: definitions)
    }

    // MARK: Definition

    mutating func parseDefinition() throws -> Definition {
        let startToken = peek()

        // Definitions must begin with a label.
        guard let labelText = checkLabel() else {
            throw ParseError.expectedDefinition(span: startToken.span)
        }
        advance() // consume the definition label

        skipNewlines()

        // Disambiguate: colon means no type annotation; another label means type annotation.
        let typeAnnotation: Expr?
        if check(.colon) {
            typeAnnotation = nil
            advance() // consume ':'
        } else if let typeName = checkLabel() {
            let typeToken = advance() // consume the type label
            typeAnnotation = .name(typeName, typeToken.span)
            skipNewlines()
            guard check(.colon) else {
                throw ParseError.expectedColon(span: peek().span)
            }
            advance() // consume ':'
        } else {
            throw ParseError.expectedColon(span: peek().span)
        }

        skipNewlines()

        let value = try parseExpr()
        let span = Span(start: startToken.span.start, end: spanOf(value).end)

        return Definition(label: labelText, typeAnnotation: typeAnnotation, value: value, span: span)
    }

    // MARK: Expr

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
            advance() // consume '('
            skipNewlines()
            let next = peek()
            if case .rightParen = next.kind {
                let closeToken = advance()
                return .unit(Span(start: token.span.start, end: closeToken.span.end))
            }
            // Non-empty parens are not supported in this phase.
            throw ParseError.unexpectedToken(span: next.span)

        default:
            throw ParseError.expectedExpression(span: token.span)
        }
    }

    // MARK: Helpers

    /// Extract the span from any `Expr` node.
    func spanOf(_ expr: Expr) -> Span {
        switch expr {
        case .integerLiteral(_, let span): return span
        case .floatLiteral(_, let span):   return span
        case .stringLiteral(_, let span):  return span
        case .interpolatedStringLiteral(_, let span): return span
        case .name(_, let span):           return span
        case .discard(let span):           return span
        case .unit(let span):              return span
        }
    }
}
