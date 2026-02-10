use crate::ast::Program;
use crate::errors::ParseError;
use crate::lexer::Token;

/// Parse a token stream into an AST [`Program`].
///
/// For now this simply produces an empty program so the pipeline compiles.
pub fn parse(_tokens: &[Token]) -> Result<Program, ParseError> {
    // TODO: implement real parsing here.
    Ok(Program { items: vec![] })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::errors::Span;
    use crate::lexer::{Token, TokenKind};

    fn eof_token(pos: usize) -> Token {
        Token {
            kind: TokenKind::Eof,
            span: Span {
                start: pos,
                end: pos,
            },
        }
    }

    #[test]
    fn parse_eof_only() {
        let tokens = vec![eof_token(0)];
        let program = parse(&tokens).expect("parsing should succeed");
        assert!(program.items.is_empty());
    }

    #[test]
    fn parse_empty_slice() {
        let tokens: Vec<Token> = vec![];
        let program = parse(&tokens).expect("parsing empty token list should succeed");
        assert!(program.items.is_empty());
    }
}
