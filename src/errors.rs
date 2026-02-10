use thiserror::Error;

/// Span representing a byte range in the source text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

/// Top-level compilation error that wraps phase-specific errors.
#[derive(Debug, Error)]
pub enum CompileError {
    #[error("Lexer error: {0}")]
    Lexer(#[from] LexError),

    #[error("Parse error: {0}")]
    Parse(#[from] ParseError),

    #[error("Codegen error: {0}")]
    Codegen(#[from] CodegenError),
}

#[derive(Debug, Error)]
pub enum LexError {
    #[error("unexpected character '{ch}' at byte {pos}")]
    UnexpectedCharacter { ch: char, pos: usize },
}

#[derive(Debug, Error)]
pub enum ParseError {
    #[error("unexpected token at {span:?}")]
    UnexpectedToken { span: Span },
}

#[derive(Debug, Error)]
pub enum CodegenError {
    #[error("LLVM error: {message}")]
    LlvmError { message: String },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lex_error_display() {
        let err = LexError::UnexpectedCharacter { ch: '@', pos: 5 };
        assert_eq!(err.to_string(), "unexpected character '@' at byte 5");
    }

    #[test]
    fn parse_error_display() {
        let err = ParseError::UnexpectedToken {
            span: Span { start: 0, end: 3 },
        };
        let msg = err.to_string();
        assert!(msg.contains("unexpected token"), "got: {msg}");
    }

    #[test]
    fn codegen_error_display() {
        let err = CodegenError::LlvmError {
            message: "bad IR".into(),
        };
        assert_eq!(err.to_string(), "LLVM error: bad IR");
    }

    #[test]
    fn compile_error_from_lex() {
        let lex_err = LexError::UnexpectedCharacter { ch: '#', pos: 0 };
        let compile_err: CompileError = lex_err.into();
        assert!(matches!(compile_err, CompileError::Lexer(_)));
    }

    #[test]
    fn compile_error_from_parse() {
        let parse_err = ParseError::UnexpectedToken {
            span: Span { start: 0, end: 1 },
        };
        let compile_err: CompileError = parse_err.into();
        assert!(matches!(compile_err, CompileError::Parse(_)));
    }

    #[test]
    fn compile_error_from_codegen() {
        let cg_err = CodegenError::LlvmError {
            message: "oops".into(),
        };
        let compile_err: CompileError = cg_err.into();
        assert!(matches!(compile_err, CompileError::Codegen(_)));
    }

    #[test]
    fn span_equality() {
        let a = Span { start: 0, end: 5 };
        let b = Span { start: 0, end: 5 };
        assert_eq!(a, b);
    }
}
