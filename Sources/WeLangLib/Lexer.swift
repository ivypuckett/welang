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
public enum TokenKind: Equatable {
    // Literals
    case integerLiteral(String)
    case floatLiteral(String)
    case stringLiteral(String)
    case interpolatedStringLiteral(String)

    // Interpolated string structure tokens (emitted starting Phase 5)
    case interpStart
    case stringSegment(String)
    case interpExprOpen
    case interpExprClose
    case interpEnd

    // Identifiers and labels
    case label(String)
    case discard

    // Punctuation and delimiters
    case colon
    case comma
    case dot
    case pipe
    case leftParen
    case rightParen
    case leftBrace
    case rightBrace
    case leftBracket
    case rightBracket
    case at
    case star
    case tick
    case newline

    // End of file
    case eof
}

/// Lex the source string into an array of `Token`s.
///
/// The returned token stream always ends with an `eof` token.
public func lex(_ source: String) throws -> [Token] {
    var lexer = Lexer(source: source)
    return try lexer.scanAll()
}

// MARK: - Internal Lexer

struct Lexer {
    let source: [UInt8]
    var pos: Int = 0

    init(source: String) {
        self.source = Array(source.utf8)
    }

    // MARK: - Scanning

    mutating func scanAll() throws -> [Token] {
        var tokens: [Token] = []

        while pos < source.count {
            if let token = try scanToken(&tokens) {
                tokens.append(token)
            }
        }

        // Always end with EOF
        tokens.append(Token(kind: .eof, span: Span(start: source.count, end: source.count)))
        return tokens
    }

    /// Scans the next token. Returns nil if the current bytes were consumed
    /// without producing a token (whitespace, comments).
    /// The `tokens` parameter is passed to check the last emitted token for
    /// newline collapsing.
    mutating func scanToken(_ tokens: inout [Token]) throws -> Token? {
        let ch = source[pos]

        // 1. Whitespace (space, tab, CR): skip
        if ch == 0x20 || ch == 0x09 || ch == 0x0D {
            pos += 1
            return nil
        }

        // 2. Newlines: emit .newline, collapsing consecutive newlines
        if ch == 0x0A {
            let start = pos
            pos += 1
            // Collapse consecutive newlines (possibly separated by whitespace/comments)
            collapseNewlines()
            // Only emit if the last token isn't already a newline
            if let last = tokens.last, case .newline = last.kind {
                return nil
            }
            return Token(kind: .newline, span: Span(start: start, end: start + 1))
        }

        // 3. Comments: consume to end of line
        if ch == 0x23 { // '#'
            skipComment()
            return nil
        }

        // 4. Standard strings
        if ch == 0x22 { // '"'
            return try scanString()
        }

        // 5. Interpolated strings (backtick)
        if ch == 0x60 { // '`'
            return try scanInterpolatedString()
        }

        // 6. Negative numbers: '-' followed by a digit
        if ch == 0x2D { // '-'
            if pos + 1 < source.count && isDigit(source[pos + 1]) {
                return try scanNumber()
            }
            // '-' not followed by digit is unexpected
            throw LexError.unexpectedCharacter(ch: character(at: pos), pos: pos)
        }

        // 7. Digits: integer or float literal
        if isDigit(ch) {
            return try scanNumber()
        }

        // 8. Labels/discard
        if isLabelStart(ch) {
            return scanLabel()
        }

        // 9. Single-character punctuation
        if let kind = singleCharToken(ch) {
            let start = pos
            pos += 1
            return Token(kind: kind, span: Span(start: start, end: pos))
        }

        // 10. Unexpected character
        throw LexError.unexpectedCharacter(ch: character(at: pos), pos: pos)
    }

    // MARK: - Newline Collapsing

    /// Consume any following whitespace, comments, and newlines so that
    /// consecutive blank lines collapse into a single .newline token.
    mutating func collapseNewlines() {
        while pos < source.count {
            let ch = source[pos]
            if ch == 0x20 || ch == 0x09 || ch == 0x0D {
                pos += 1
            } else if ch == 0x0A {
                pos += 1
            } else if ch == 0x23 { // '#'
                skipComment()
            } else {
                break
            }
        }
    }

    // MARK: - Comment

    mutating func skipComment() {
        // Consume from '#' through (but not including) the next '\n' or EOF
        while pos < source.count && source[pos] != 0x0A {
            pos += 1
        }
    }

    // MARK: - String Scanning

    mutating func scanString() throws -> Token {
        let start = pos
        pos += 1 // skip opening '"'

        var value: [UInt8] = []

        while pos < source.count {
            let ch = source[pos]

            if ch == 0x22 { // closing '"'
                pos += 1
                let s = String(bytes: value, encoding: .utf8) ?? ""
                return Token(kind: .stringLiteral(s), span: Span(start: start, end: pos))
            }

            if ch == 0x5C { // backslash
                pos += 1
                if pos >= source.count {
                    throw LexError.unterminatedString(pos: start)
                }
                let escaped = source[pos]
                switch escaped {
                case 0x5C: value.append(0x5C) // \\
                case 0x22: value.append(0x22) // \"
                case 0x6E: value.append(0x0A) // \n
                case 0x74: value.append(0x09) // \t
                case 0x72: value.append(0x0D) // \r
                case 0x30: value.append(0x00) // \0
                default:
                    throw LexError.invalidEscape(ch: character(at: pos), pos: pos)
                }
                pos += 1
                continue
            }

            value.append(ch)
            pos += 1
        }

        throw LexError.unterminatedString(pos: start)
    }

    // MARK: - Interpolated String Scanning (Phase 1 stub)

    mutating func scanInterpolatedString() throws -> Token {
        let start = pos
        pos += 1 // skip opening '`'

        var content: [UInt8] = []

        while pos < source.count {
            let ch = source[pos]

            if ch == 0x60 { // closing '`'
                pos += 1
                let s = String(bytes: content, encoding: .utf8) ?? ""
                return Token(kind: .interpolatedStringLiteral(s), span: Span(start: start, end: pos))
            }

            if ch == 0x5C { // backslash — validate escape
                pos += 1
                if pos >= source.count {
                    throw LexError.unterminatedInterpolatedString(pos: start)
                }
                let escaped = source[pos]
                switch escaped {
                case 0x7B: // \{
                    content.append(0x5C)
                    content.append(0x7B)
                case 0x5C: // \\
                    content.append(0x5C)
                    content.append(0x5C)
                case 0x60: // \`
                    content.append(0x5C)
                    content.append(0x60)
                default:
                    throw LexError.invalidEscape(ch: character(at: pos), pos: pos)
                }
                pos += 1
                continue
            }

            content.append(ch)
            pos += 1
        }

        throw LexError.unterminatedInterpolatedString(pos: start)
    }

    // MARK: - Number Scanning

    mutating func scanNumber() throws -> Token {
        let start = pos

        // Consume optional leading '-'
        if source[pos] == 0x2D {
            pos += 1
        }

        // Consume digits
        while pos < source.count && isDigit(source[pos]) {
            pos += 1
        }

        // Check for '.' followed by digit → float
        if pos < source.count && source[pos] == 0x2E {
            if pos + 1 < source.count && isDigit(source[pos + 1]) {
                pos += 1 // skip '.'
                while pos < source.count && isDigit(source[pos]) {
                    pos += 1
                }
                let raw = String(bytes: Array(source[start..<pos]), encoding: .utf8) ?? ""
                return Token(kind: .floatLiteral(raw), span: Span(start: start, end: pos))
            }
        }

        let raw = String(bytes: Array(source[start..<pos]), encoding: .utf8) ?? ""
        return Token(kind: .integerLiteral(raw), span: Span(start: start, end: pos))
    }

    // MARK: - Label Scanning

    mutating func scanLabel() -> Token {
        let start = pos
        pos += 1 // consume first character

        while pos < source.count && isLabelContinue(source[pos]) {
            pos += 1
        }

        let raw = String(bytes: Array(source[start..<pos]), encoding: .utf8) ?? ""

        if raw == "_" {
            return Token(kind: .discard, span: Span(start: start, end: pos))
        }

        return Token(kind: .label(raw), span: Span(start: start, end: pos))
    }

    // MARK: - Single-character Tokens

    func singleCharToken(_ ch: UInt8) -> TokenKind? {
        switch ch {
        case 0x28: return .leftParen     // (
        case 0x29: return .rightParen    // )
        case 0x7B: return .leftBrace     // {
        case 0x7D: return .rightBrace    // }
        case 0x5B: return .leftBracket   // [
        case 0x5D: return .rightBracket  // ]
        case 0x3A: return .colon         // :
        case 0x2C: return .comma         // ,
        case 0x2E: return .dot           // .
        case 0x7C: return .pipe          // |
        case 0x40: return .at            // @
        case 0x2A: return .star          // *
        case 0x27: return .tick          // '
        default: return nil
        }
    }

    // MARK: - Character Helpers

    func isDigit(_ ch: UInt8) -> Bool {
        ch >= 0x30 && ch <= 0x39
    }

    func isLabelStart(_ ch: UInt8) -> Bool {
        (ch >= 0x61 && ch <= 0x7A) || // a-z
        (ch >= 0x41 && ch <= 0x5A) || // A-Z
        ch == 0x5F                     // _
    }

    func isLabelContinue(_ ch: UInt8) -> Bool {
        isLabelStart(ch) || isDigit(ch)
    }

    /// Convert a byte at a given position to a Swift `Character` for error reporting.
    func character(at index: Int) -> Character {
        let byte = source[index]
        return Character(UnicodeScalar(byte))
    }
}
