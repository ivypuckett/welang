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

    public var description: String {
        switch self {
        case .unexpectedCharacter(let ch, let pos):
            return "unexpected character '\(ch)' at byte \(pos)"
        }
    }
}

public enum ParseError: Error, Equatable, CustomStringConvertible {
    case unexpectedToken(span: Span)

    public var description: String {
        switch self {
        case .unexpectedToken(let span):
            return "unexpected token at \(span)"
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
