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

    /// Expr = "(" ... ")"  |  Atom
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

    // MARK: Parenthesized Expressions

    /// Parses a parenthesized expression:
    ///   "()"               → .unit
    ///   "(label: body)"    → .lambda
    ///   "(| clause ...)"   → .pipe with implicit x as first clause
    ///   "(PipeExpr)"       → .apply or .pipe or unwrapped single expr
    private mutating func parseParen() throws -> Expr {
        let open = advance() // consume '('
        skipNewlines()

        // Unit: ()
        if let close = match(.rightParen) {
            return .unit(Span(start: open.span.start, end: close.span.end))
        }

        // Leading pipe: (| clause (| clause)*)
        if peek().kind == .pipe {
            advance() // consume '|'
            skipNewlines()
            let implicitX = Expr.name("x", open.span)
            var clauses: [Expr] = [implicitX]
            clauses.append(try parseClause())
            skipNewlines()
            while peek().kind == .pipe {
                advance() // consume '|'
                skipNewlines()
                clauses.append(try parseClause())
                skipNewlines()
            }
            guard let close = match(.rightParen) else {
                throw ParseError.expectedClosingParen(span: peek().span)
            }
            return .pipe(clauses: clauses, Span(start: open.span.start, end: close.span.end))
        }

        // Lambda with named parameter: (label: body)
        // Peek: if current is a label and next (after possible newlines) is colon → lambda.
        if case .label(let paramName) = peek().kind {
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
            } else {
                // Not a lambda — backtrack and fall through to PipeExpr
                pos = savedPos
            }
        }

        // Otherwise: parse as PipeExpr (s-expression or pipe)
        let expr = try parsePipeExpr()
        skipNewlines()
        guard let _ = match(.rightParen) else {
            throw ParseError.expectedClosingParen(span: peek().span)
        }
        return expr
    }

    // MARK: PipeExpr

    /// PipeExpr = Clause ("|" Clause)*
    ///
    /// Returns a single Expr (unwrapped if one clause, .pipe if multiple).
    private mutating func parsePipeExpr() throws -> Expr {
        let firstClause = try parseClause()
        skipNewlines()

        guard peek().kind == .pipe else {
            // Single clause — return as-is (already unwrapped by parseClause)
            return firstClause
        }

        // Multiple clauses — build a pipe
        var clauses: [Expr] = [firstClause]
        while peek().kind == .pipe {
            advance() // consume '|'
            skipNewlines()
            clauses.append(try parseClause())
            skipNewlines()
        }

        let span = Span(start: clauses.first!.span.start, end: clauses.last!.span.end)
        return .pipe(clauses: clauses, span)
    }

    // MARK: Clause

    /// Clause = Atom+
    ///
    /// Collects atoms until `)`, `|`, or EOF. Returns a single Expr if one atom,
    /// or `.apply(function: first, arguments: rest)` if multiple.
    private mutating func parseClause() throws -> Expr {
        var atoms: [Expr] = []
        skipNewlines()
        while peek().kind != .rightParen && peek().kind != .pipe && peek().kind != .eof {
            atoms.append(try parseAtom())
            skipNewlines()
        }
        guard !atoms.isEmpty else {
            throw ParseError.emptyClause(span: peek().span)
        }
        if atoms.count == 1 {
            return atoms[0]
        }
        let span = Span(start: atoms.first!.span.start, end: atoms.last!.span.end)
        return .apply(function: atoms[0], arguments: Array(atoms.dropFirst()), span)
    }
}
