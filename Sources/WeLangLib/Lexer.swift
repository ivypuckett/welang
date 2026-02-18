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

// MARK: - ASCII Byte Constants

private enum Ascii {
    // Whitespace
    static let tab:       UInt8 = 0x09
    static let lineFeed:  UInt8 = 0x0A
    static let carriageReturn: UInt8 = 0x0D
    static let space:     UInt8 = 0x20

    // Digits
    static let zero:      UInt8 = 0x30  // '0'
    static let nine:      UInt8 = 0x39  // '9'

    // Uppercase letters
    static let upperA:    UInt8 = 0x41
    static let upperZ:    UInt8 = 0x5A

    // Lowercase letters
    static let lowerA:    UInt8 = 0x61
    static let lowerZ:    UInt8 = 0x7A

    // Punctuation & delimiters
    static let doubleQuote: UInt8 = 0x22  // "
    static let hash:      UInt8 = 0x23  // #
    static let singleQuote: UInt8 = 0x27  // '
    static let leftParen: UInt8 = 0x28  // (
    static let rightParen: UInt8 = 0x29  // )
    static let star:      UInt8 = 0x2A  // *
    static let comma:     UInt8 = 0x2C  // ,
    static let dash:      UInt8 = 0x2D  // -
    static let dot:       UInt8 = 0x2E  // .
    static let colon:     UInt8 = 0x3A  // :
    static let at:        UInt8 = 0x40  // @
    static let leftBracket: UInt8 = 0x5B  // [
    static let backslash: UInt8 = 0x5C  // \
    static let rightBracket: UInt8 = 0x5D  // ]
    static let underscore: UInt8 = 0x5F  // _
    static let backtick:  UInt8 = 0x60  // `
    static let leftBrace: UInt8 = 0x7B  // {
    static let pipe:      UInt8 = 0x7C  // |
    static let rightBrace: UInt8 = 0x7D  // }

    // Escape result bytes
    static let null:      UInt8 = 0x00

    // Escape sequence source characters (the char after '\')
    static let charN:     UInt8 = 0x6E  // 'n'
    static let charT:     UInt8 = 0x74  // 't'
    static let charR:     UInt8 = 0x72  // 'r'
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
        if ch == Ascii.space || ch == Ascii.tab || ch == Ascii.carriageReturn {
            pos += 1
            return nil
        }

        // 2. Newlines: emit .newline, collapsing consecutive newlines
        if ch == Ascii.lineFeed {
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
        if ch == Ascii.hash {
            skipComment()
            return nil
        }

        // 4. Standard strings
        if ch == Ascii.doubleQuote {
            return try scanString()
        }

        // 5. Interpolated strings (backtick)
        if ch == Ascii.backtick {
            return try scanInterpolatedString()
        }

        // 6. Negative numbers: '-' followed by a digit
        if ch == Ascii.dash {
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
            if ch == Ascii.space || ch == Ascii.tab || ch == Ascii.carriageReturn {
                pos += 1
            } else if ch == Ascii.lineFeed {
                pos += 1
            } else if ch == Ascii.hash {
                skipComment()
            } else {
                break
            }
        }
    }

    // MARK: - Comment

    mutating func skipComment() {
        // Consume from '#' through (but not including) the next '\n' or EOF
        while pos < source.count && source[pos] != Ascii.lineFeed {
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

            if ch == Ascii.doubleQuote { // closing "
                pos += 1
                let s = String(bytes: value, encoding: .utf8) ?? ""
                return Token(kind: .stringLiteral(s), span: Span(start: start, end: pos))
            }

            if ch == Ascii.backslash {
                pos += 1
                if pos >= source.count {
                    throw LexError.unterminatedString(pos: start)
                }
                let escaped = source[pos]
                switch escaped {
                case Ascii.backslash:    value.append(Ascii.backslash)    // \\
                case Ascii.doubleQuote:  value.append(Ascii.doubleQuote)  // \"
                case Ascii.charN:        value.append(Ascii.lineFeed)     // \n
                case Ascii.charT:        value.append(Ascii.tab)          // \t
                case Ascii.charR:        value.append(Ascii.carriageReturn) // \r
                case Ascii.zero:         value.append(Ascii.null)         // \0
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

            if ch == Ascii.backtick { // closing `
                pos += 1
                let s = String(bytes: content, encoding: .utf8) ?? ""
                return Token(kind: .interpolatedStringLiteral(s), span: Span(start: start, end: pos))
            }

            if ch == Ascii.backslash {
                pos += 1
                if pos >= source.count {
                    throw LexError.unterminatedInterpolatedString(pos: start)
                }
                let escaped = source[pos]
                switch escaped {
                case Ascii.leftBrace: // \{
                    content.append(Ascii.backslash)
                    content.append(Ascii.leftBrace)
                case Ascii.backslash: // \\
                    content.append(Ascii.backslash)
                    content.append(Ascii.backslash)
                case Ascii.backtick: // \`
                    content.append(Ascii.backslash)
                    content.append(Ascii.backtick)
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
        if source[pos] == Ascii.dash {
            pos += 1
        }

        // Consume digits
        while pos < source.count && isDigit(source[pos]) {
            pos += 1
        }

        // Check for '.' followed by digit → float
        if pos < source.count && source[pos] == Ascii.dot {
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
        case Ascii.leftParen:    return .leftParen
        case Ascii.rightParen:   return .rightParen
        case Ascii.leftBrace:    return .leftBrace
        case Ascii.rightBrace:   return .rightBrace
        case Ascii.leftBracket:  return .leftBracket
        case Ascii.rightBracket: return .rightBracket
        case Ascii.colon:        return .colon
        case Ascii.comma:        return .comma
        case Ascii.dot:          return .dot
        case Ascii.pipe:         return .pipe
        case Ascii.at:           return .at
        case Ascii.star:         return .star
        case Ascii.singleQuote:  return .tick
        default: return nil
        }
    }

    // MARK: - Character Helpers

    func isDigit(_ ch: UInt8) -> Bool {
        ch >= Ascii.zero && ch <= Ascii.nine
    }

    func isLabelStart(_ ch: UInt8) -> Bool {
        (ch >= Ascii.lowerA && ch <= Ascii.lowerZ) ||
        (ch >= Ascii.upperA && ch <= Ascii.upperZ) ||
        ch == Ascii.underscore
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
