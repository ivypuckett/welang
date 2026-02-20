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

    /// Expr = Atom | "(" ... ")"
    mutating func parseExpr() throws -> Expr {
        if peek().kind == .leftParen {
            return try parseParen()
        }
        return try parseAtom()
    }

    // MARK: Atom

    /// Atom = IntegerLiteral | FloatLiteral | StringLiteral
    ///      | InterpolatedStringLiteral | Label | "_" | "(" ... ")"
    mutating func parseAtom() throws -> Expr {
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
            return try parseParen()

        default:
            throw ParseError.expectedExpression(span: token.span)
        }
    }

    // MARK: Paren

    /// Parses a parenthesized expression:
    ///   "()"                         → unit
    ///   "(" Label ":" PipeExpr ")"   → lambda
    ///   "(" "|" Clause ... ")"       → pipe with leading implicit x
    ///   "(" PipeExpr ")"             → s-expression or pipe
    private mutating func parseParen() throws -> Expr {
        let open = advance() // consume '('
        skipNewlines()

        // Unit: ()
        if let close = match(.rightParen) {
            return .unit(Span(start: open.span.start, end: close.span.end))
        }

        // Leading pipe: (| ...)
        if peek().kind == .pipe {
            let pipeToken = advance() // consume '|'
            skipNewlines()
            // Insert implicit x as first clause
            let implicitX = Expr.name("x", Span(start: pipeToken.span.start, end: pipeToken.span.start))
            var clauses: [Expr] = [implicitX]

            // Parse the first clause after leading pipe
            let firstClause = try parseClause()
            clauses.append(firstClause)

            // Continue parsing pipe-separated clauses
            skipNewlines()
            while peek().kind == .pipe {
                advance() // consume '|'
                skipNewlines()
                if peek().kind == .pipe || peek().kind == .rightParen {
                    throw ParseError.emptyClause(span: peek().span)
                }
                clauses.append(try parseClause())
                skipNewlines()
            }

            guard let close = match(.rightParen) else {
                throw ParseError.expectedClosingParen(span: peek().span)
            }
            return .pipe(clauses: clauses, Span(start: open.span.start, end: close.span.end))
        }

        // Lambda disambiguation: label followed by colon inside parens
        if case .label(let paramName) = peek().kind {
            // Look ahead: is the next non-newline token a colon?
            let savedPos = pos
            advance() // consume label
            skipNewlines()
            if peek().kind == .colon {
                advance() // consume ':'
                skipNewlines()
                let body = try parsePipeExpr()
                skipNewlines()
                guard let close = match(.rightParen) else {
                    throw ParseError.expectedClosingParen(span: peek().span)
                }
                return .lambda(
                    param: paramName,
                    body: body,
                    Span(start: open.span.start, end: close.span.end)
                )
            }
            // Not a lambda — backtrack and parse as normal pipe expression
            pos = savedPos
        }

        // General case: PipeExpr
        let expr = try parsePipeExpr()
        skipNewlines()
        guard let close = match(.rightParen) else {
            throw ParseError.expectedClosingParen(span: peek().span)
        }

        // Re-span the expression to include the parens for single-element case
        return respanIfSingle(expr, open: open, close: close)
    }

    // MARK: PipeExpr

    /// PipeExpr = Clause ("|" Clause)*
    ///
    /// If there is only one clause with one element, returns that element directly.
    /// If there is one clause with multiple elements, returns .apply.
    /// If there are multiple clauses (pipe-separated), returns .pipe.
    private mutating func parsePipeExpr() throws -> Expr {
        let firstClause = try parseClause()
        skipNewlines()

        // Check for pipes
        guard peek().kind == .pipe else {
            return firstClause
        }

        var clauses: [Expr] = [firstClause]
        while peek().kind == .pipe {
            advance() // consume '|'
            skipNewlines()
            if peek().kind == .pipe || peek().kind == .rightParen {
                throw ParseError.emptyClause(span: peek().span)
            }
            clauses.append(try parseClause())
            skipNewlines()
        }

        let span = Span(start: clauses.first!.span.start, end: clauses.last!.span.end)
        return .pipe(clauses: clauses, span)
    }

    // MARK: Clause

    /// Clause = Atom+
    ///
    /// Collects atoms until ')' or '|' or EOF is seen.
    /// Single atom → returned directly.
    /// Multiple atoms → .apply(function: first, arguments: rest).
    private mutating func parseClause() throws -> Expr {
        var atoms: [Expr] = []

        while !isClauseTerminator(peek().kind) {
            atoms.append(try parseAtom())
            skipNewlines()
        }

        guard !atoms.isEmpty else {
            throw ParseError.emptyClause(span: peek().span)
        }

        if atoms.count == 1 {
            return atoms[0]
        }

        let function = atoms[0]
        let arguments = Array(atoms[1...])
        let span = Span(start: function.span.start, end: arguments.last!.span.end)
        return .apply(function: function, arguments: arguments, span)
    }

    /// Returns true if the token kind terminates a clause.
    /// Newlines are not terminators because clauses only appear inside parens
    /// where newlines are insignificant.
    private func isClauseTerminator(_ kind: TokenKind) -> Bool {
        switch kind {
        case .pipe, .rightParen, .eof:
            return true
        default:
            return false
        }
    }

    /// For single-element parens like (x), the inner expression is returned
    /// without wrapping. We don't re-span it — the span comes from the inner expression.
    /// For multi-element or pipe expressions, the outer parens span is already captured.
    private func respanIfSingle(_ expr: Expr, open: Token, close: Token) -> Expr {
        return expr
    }
}
