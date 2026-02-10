use crate::errors::{LexError, Span};

/// A single lexical token produced by the lexer.
#[derive(Debug, Clone, PartialEq)]
pub struct Token {
    pub kind: TokenKind,
    pub span: Span,
}

/// The kind of a lexical token.
///
/// This enum will be extended as the language grows. For now it only
/// contains `Eof` so that the scaffolding compiles and tests pass.
#[derive(Debug, Clone, PartialEq)]
pub enum TokenKind {
    /// End-of-file marker.
    Eof,
}

/// Lex the source string into a vector of [`Token`]s.
///
/// The returned token stream always ends with an `Eof` token.
pub fn lex(source: &str) -> Result<Vec<Token>, LexError> {
    // TODO: implement real tokenization here.
    // For now, just produce Eof so downstream stages have something to work with.

    let tokens = vec![Token {
        kind: TokenKind::Eof,
        span: Span {
            start: source.len(),
            end: source.len(),
        },
    }];

    Ok(tokens)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lex_empty_source_returns_eof() {
        let tokens = lex("").expect("lexing empty source should succeed");
        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0].kind, TokenKind::Eof);
        assert_eq!(tokens[0].span, Span { start: 0, end: 0 });
    }

    #[test]
    fn lex_returns_eof_at_end() {
        let tokens = lex("hello").expect("lexing should succeed");
        let last = tokens.last().expect("should have at least one token");
        assert_eq!(last.kind, TokenKind::Eof);
    }

    #[test]
    fn lex_eof_span_matches_source_length() {
        let source = "abc";
        let tokens = lex(source).unwrap();
        let eof = tokens.last().unwrap();
        assert_eq!(eof.span.start, source.len());
    }
}
