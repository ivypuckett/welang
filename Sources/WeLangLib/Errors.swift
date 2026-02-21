import Foundation

/// Span representing a byte range in the source text.
public struct Span: Equatable, CustomStringConvertible {
    public let start: Int
    public let end: Int

    public init(start: Int, end: Int) {
        self.start = start
        self.end = end
    }

    public var description: String {
        "Span(start: \(start), end: \(end))"
    }
}

// MARK: - Error Types

/// Top-level compilation error that wraps phase-specific errors.
public enum CompileError: Error, CustomStringConvertible {
    case lexer(LexError)
    case parse(ParseError)
    case codegen(CodegenError)

    public var description: String {
        switch self {
        case .lexer(let e): return "Lexer error: \(e)"
        case .parse(let e): return "Parse error: \(e)"
        case .codegen(let e): return "Codegen error: \(e)"
        }
    }
}

public enum LexError: Error, Equatable, CustomStringConvertible {
    case unexpectedCharacter(ch: Character, pos: Int)
    case invalidEscape(ch: Character, pos: Int)
    case unterminatedString(pos: Int)
    case unterminatedInterpolatedString(pos: Int)
    case unterminatedInterpolation(pos: Int)

    public var description: String {
        switch self {
        case .unexpectedCharacter(let ch, let pos):
            return "unexpected character '\(ch)' at byte \(pos)"
        case .invalidEscape(let ch, let pos):
            return "invalid escape sequence '\\(\(ch))' at byte \(pos)"
        case .unterminatedString(let pos):
            return "unterminated string literal starting at byte \(pos)"
        case .unterminatedInterpolatedString(let pos):
            return "unterminated interpolated string starting at byte \(pos)"
        case .unterminatedInterpolation(let pos):
            return "unterminated interpolation starting at byte \(pos)"
        }
    }
}

public enum ParseError: Error, Equatable, CustomStringConvertible {
    case unexpectedToken(span: Span)
    case expectedColon(span: Span)
    case expectedExpression(span: Span)
    case expectedDefinition(span: Span)
    case expectedClosingParen(span: Span)
    case emptyClause(span: Span)

    public var description: String {
        switch self {
        case .unexpectedToken(let span):
            return "unexpected token at \(span)"
        case .expectedColon(let span):
            return "expected ':' at \(span)"
        case .expectedExpression(let span):
            return "expected expression at \(span)"
        case .expectedDefinition(let span):
            return "expected definition at \(span)"
        case .expectedClosingParen(let span):
            return "expected closing ')' at \(span)"
        case .emptyClause(let span):
            return "empty clause at \(span)"
        }
    }
}

public enum CodegenError: Error, Equatable, CustomStringConvertible {
    case llvmError(message: String)

    public var description: String {
        switch self {
        case .llvmError(let message):
            return "LLVM error: \(message)"
        }
    }
}
