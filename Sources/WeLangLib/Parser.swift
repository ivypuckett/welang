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

    /// Expr = "(" ... ")"  |  PostfixExpr
    mutating func parseExpr() throws -> Expr {
        return try parsePostfixExpr()
    }

    // MARK: PostfixExpr

    /// PostfixExpr = Atom Accessor*
    /// Accessor = "." Label | "." "[" Expr "]" | "[" Expr "]"
    private mutating func parsePostfixExpr() throws -> Expr {
        var expr = try parseAtom()

        while true {
            if peek().kind == .dot {
                let dotToken = advance() // consume '.'
                skipNewlines()

                if peek().kind == .leftBracket {
                    // Computed access: x.[ expr ]
                    advance() // consume '['
                    skipNewlines()
                    let index = try parseExpr()
                    skipNewlines()
                    guard let close = match(.rightBracket) else {
                        throw ParseError.expectedClosingBracket(span: peek().span)
                    }
                    expr = .computedAccess(
                        expr: expr,
                        index: index,
                        Span(start: expr.span.start, end: close.span.end)
                    )
                } else if case .label(let fieldName) = peek().kind {
                    // Dot access: x.label
                    let fieldToken = advance()
                    expr = .dotAccess(
                        expr: expr,
                        field: fieldName,
                        Span(start: expr.span.start, end: fieldToken.span.end)
                    )
                } else {
                    throw ParseError.expectedField(span: dotToken.span)
                }
            } else if peek().kind == .leftBracket {
                // Bracket access: x[0]
                advance() // consume '['
                skipNewlines()
                let index = try parseExpr()
                skipNewlines()
                guard let close = match(.rightBracket) else {
                    throw ParseError.expectedClosingBracket(span: peek().span)
                }
                expr = .bracketAccess(
                    expr: expr,
                    index: index,
                    Span(start: expr.span.start, end: close.span.end)
                )
            } else {
                break
            }
        }

        return expr
    }

    // MARK: Atom

    /// Atom = IntegerLiteral | FloatLiteral | StringLiteral
    ///      | InterpolatedStringLiteral | Label | "_" | "(" ... ")"
    ///      | "{" ... "}" | "[" ... "]"
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

        case .leftBrace:
            return try parseTupleLiteral()

        case .leftBracket:
            return try parseArrayLiteral()

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

    /// Clause = PostfixExpr+
    ///
    /// Collects postfix expressions until `)`, `|`, or EOF. Returns a single Expr if one,
    /// or `.apply(function: first, arguments: rest)` if multiple.
    private mutating func parseClause() throws -> Expr {
        var atoms: [Expr] = []
        skipNewlines()
        while peek().kind != .rightParen && peek().kind != .pipe && peek().kind != .eof {
            atoms.append(try parsePostfixExpr())
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

    // MARK: Tuple Literal

    /// TupleLiteral = "{" EntryList? "}"
    /// EntryList = Entry ("," Entry)* ","?
    /// Entry = (Key ":")? Expr
    private mutating func parseTupleLiteral() throws -> Expr {
        let open = advance() // consume '{'
        skipNewlines()

        // Empty tuple: {}
        if let close = match(.rightBrace) {
            return .tuple(entries: [], Span(start: open.span.start, end: close.span.end))
        }

        var entries: [CompoundEntry] = []
        entries.append(try parseCompoundEntry())
        skipNewlines()

        while match(.comma) != nil {
            skipNewlines()
            // Allow trailing comma: check for closing brace after comma
            if peek().kind == .rightBrace { break }
            entries.append(try parseCompoundEntry())
            skipNewlines()
        }

        guard let close = match(.rightBrace) else {
            throw ParseError.expectedClosingBrace(span: peek().span)
        }

        return .tuple(entries: entries, Span(start: open.span.start, end: close.span.end))
    }

    // MARK: Array Literal

    /// ArrayLiteral = "[" EntryList? "]"
    /// EntryList = Entry ("," Entry)* ","?
    /// Entry = (Key ":")? Expr
    private mutating func parseArrayLiteral() throws -> Expr {
        let open = advance() // consume '['
        skipNewlines()

        // Empty array: []
        if let close = match(.rightBracket) {
            return .array(entries: [], Span(start: open.span.start, end: close.span.end))
        }

        var entries: [CompoundEntry] = []
        entries.append(try parseCompoundEntry())
        skipNewlines()

        while match(.comma) != nil {
            skipNewlines()
            // Allow trailing comma: check for closing bracket after comma
            if peek().kind == .rightBracket { break }
            entries.append(try parseCompoundEntry())
            skipNewlines()
        }

        guard let close = match(.rightBracket) else {
            throw ParseError.expectedClosingBracket(span: peek().span)
        }

        return .array(entries: entries, Span(start: open.span.start, end: close.span.end))
    }

    // MARK: Compound Entry

    /// Entry = (Key ":")? Expr
    /// Key = IntegerLiteral | Label | StringLiteral
    ///
    /// Look ahead: if the current token is an integer/label/string AND the next
    /// non-newline token is ':', treat it as a keyed entry. Otherwise, implicit.
    private mutating func parseCompoundEntry() throws -> CompoundEntry {
        let startSpan = peek().span

        // Try to detect a key: IntegerLiteral, Label, or StringLiteral followed by ':'
        switch peek().kind {
        case .integerLiteral(let text):
            let savedPos = pos
            let keyToken = advance()
            skipNewlines()
            if match(.colon) != nil {
                skipNewlines()
                let value = try parseExpr()
                return CompoundEntry(
                    key: .index(text, keyToken.span),
                    value: value,
                    span: Span(start: keyToken.span.start, end: value.span.end)
                )
            }
            // Not a key — backtrack
            pos = savedPos

        case .label(let text):
            let savedPos = pos
            let keyToken = advance()
            skipNewlines()
            if match(.colon) != nil {
                skipNewlines()
                let value = try parseExpr()
                return CompoundEntry(
                    key: .label(text, keyToken.span),
                    value: value,
                    span: Span(start: keyToken.span.start, end: value.span.end)
                )
            }
            // Not a key — backtrack
            pos = savedPos

        case .stringLiteral(let text):
            let savedPos = pos
            let keyToken = advance()
            skipNewlines()
            if match(.colon) != nil {
                skipNewlines()
                let value = try parseExpr()
                return CompoundEntry(
                    key: .stringKey(text, keyToken.span),
                    value: value,
                    span: Span(start: keyToken.span.start, end: value.span.end)
                )
            }
            // Not a key — backtrack
            pos = savedPos

        default:
            break
        }

        // Implicit key: just parse the value expression
        let value = try parseExpr()
        return CompoundEntry(
            key: .implicit,
            value: value,
            span: Span(start: startSpan.start, end: value.span.end)
        )
    }
}
