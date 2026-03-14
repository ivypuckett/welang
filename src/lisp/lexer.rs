use logos::{Lexer, Logos};

/// A token produced by the LISP lexer.
#[derive(Debug, PartialEq, Clone)]
pub enum Token {
    LParen,
    RParen,
    LBracket,
    RBracket,
    LBrace,
    RBrace,
    LAngle,
    RAngle,
    Comma,
    Colon,
    Pipe,
    Dot,
    Quote,
    Star,
    Bool(bool),
    Number(f64),
    Str(String),
    Symbol(String),
}

/// The kind of error that occurred during lexing.
#[derive(Debug, PartialEq, Clone)]
pub enum LexErrorKind {
    UnterminatedString,
}

impl std::fmt::Display for LexErrorKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LexErrorKind::UnterminatedString => write!(f, "unterminated string literal"),
        }
    }
}

/// An error that occurred during lexing, with a 1-indexed line number.
#[derive(Debug, PartialEq, Clone)]
pub struct LexError {
    pub kind: LexErrorKind,
    pub line: usize,
}

impl std::fmt::Display for LexError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.kind)
    }
}

fn lex_string(lex: &mut Lexer<'_, RawToken>) -> String {
    let slice = lex.slice();
    // slice includes the surrounding quotes; strip them
    let inner = &slice[1..slice.len() - 1];
    let mut s = String::new();
    let mut chars = inner.chars();
    while let Some(ch) = chars.next() {
        if ch == '\\' {
            match chars.next() {
                Some('n') => s.push('\n'),
                Some('t') => s.push('\t'),
                Some('r') => s.push('\r'),
                Some('"') => s.push('"'),
                Some('\\') => s.push('\\'),
                _ => s.push('\\'),
            }
        } else {
            s.push(ch);
        }
    }
    s
}

fn lex_number(lex: &mut Lexer<'_, RawToken>) -> f64 {
    // Replace the first 'f' with '.' to support the `3f14` → `3.14` notation.
    lex.slice()
        .replacen('f', ".", 1)
        .parse()
        .unwrap_or(f64::INFINITY)
}

/// Internal token type used by the Logos lexer.
#[derive(Logos, Debug)]
#[logos(skip r"[ \t\r\n]+")] // skip all whitespace
#[logos(skip r"#[^\n]*")] // skip line comments
enum RawToken {
    #[token("(")]
    LParen,
    #[token(")")]
    RParen,
    #[token("[")]
    LBracket,
    #[token("]")]
    RBracket,
    #[token("{")]
    LBrace,
    #[token("}")]
    RBrace,
    #[token("<")]
    LAngle,
    #[token(">")]
    RAngle,
    #[token(",")]
    Comma,
    #[token(":")]
    Colon,
    #[token("|")]
    Pipe,
    #[token(".")]
    Dot,
    #[token("'")]
    Quote,
    #[token("*")]
    Star,

    #[token("true")]
    True,
    #[token("false")]
    False,

    /// A complete, properly-terminated string literal.
    #[regex(r#""([^"\\]|\\.)*""#, lex_string)]
    Str(String),

    /// An unterminated string literal (no closing `"`).
    #[regex(r#""([^"\\]|\\.)*"#)]
    UnterminatedStr,

    /// Integer or `f`-notation float: `42`, `-3`, `3f14` (→ `3.14`).
    /// Priority 2 ensures this beats the Symbol regex on equal-length matches.
    #[regex(r"[+-]?[0-9]+(f[0-9]*)?", lex_number, priority = 3)]
    Number(f64),

    /// Any run of characters that are not syntactically special.
    #[regex(r##"[^\s()\[\]{}<>,"#:|.*']+"##, |lex| lex.slice().to_string())]
    Symbol(String),
}

/// Return the 1-based line number for a byte offset given a pre-computed
/// table of line-start offsets.
fn line_of(line_starts: &[usize], offset: usize) -> usize {
    match line_starts.binary_search(&offset) {
        Ok(idx) => idx + 1,
        Err(idx) => idx,
    }
}

/// Tokenize a LISP source string into a list of `(token, line)` pairs.
/// Line numbers are 1-indexed.
pub fn tokenize(input: &str) -> Result<Vec<(Token, usize)>, LexError> {
    // Build a table of byte offsets at which each line starts so that we can
    // convert Logos's byte-offset spans to 1-based line numbers.
    let mut line_starts = vec![0usize];
    for (i, ch) in input.char_indices() {
        if ch == '\n' {
            line_starts.push(i + 1);
        }
    }

    let mut tokens = Vec::new();

    for (result, span) in RawToken::lexer(input).spanned() {
        let line = line_of(&line_starts, span.start);
        if let Ok(raw) = result {
            match raw {
                RawToken::LParen => tokens.push((Token::LParen, line)),
                RawToken::RParen => tokens.push((Token::RParen, line)),
                RawToken::LBracket => tokens.push((Token::LBracket, line)),
                RawToken::RBracket => tokens.push((Token::RBracket, line)),
                RawToken::LBrace => tokens.push((Token::LBrace, line)),
                RawToken::RBrace => tokens.push((Token::RBrace, line)),
                RawToken::LAngle => tokens.push((Token::LAngle, line)),
                RawToken::RAngle => tokens.push((Token::RAngle, line)),
                RawToken::Comma => tokens.push((Token::Comma, line)),
                RawToken::Colon => tokens.push((Token::Colon, line)),
                RawToken::Pipe => tokens.push((Token::Pipe, line)),
                RawToken::Dot => tokens.push((Token::Dot, line)),
                RawToken::Quote => tokens.push((Token::Quote, line)),
                RawToken::Star => tokens.push((Token::Star, line)),
                RawToken::True => tokens.push((Token::Bool(true), line)),
                RawToken::False => tokens.push((Token::Bool(false), line)),
                RawToken::Str(s) => tokens.push((Token::Str(s), line)),
                RawToken::Number(n) => tokens.push((Token::Number(n), line)),
                RawToken::Symbol(s) => tokens.push((Token::Symbol(s), line)),
                RawToken::UnterminatedStr => {
                    return Err(LexError {
                        kind: LexErrorKind::UnterminatedString,
                        line,
                    });
                }
            }
        }
    }

    Ok(tokens)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Strip line numbers from tokenize output for brevity in most tests.
    fn tok(input: &str) -> Vec<Token> {
        tokenize(input)
            .unwrap()
            .into_iter()
            .map(|(t, _)| t)
            .collect()
    }

    fn tok_err(input: &str) -> LexErrorKind {
        tokenize(input).unwrap_err().kind
    }

    #[test]
    fn test_empty_input() {
        assert_eq!(tok(""), vec![]);
    }

    #[test]
    fn test_parens() {
        assert_eq!(tok("()"), vec![Token::LParen, Token::RParen]);
    }

    #[test]
    fn test_integer() {
        assert_eq!(tok("42"), vec![Token::Number(42.0)]);
    }

    #[test]
    fn test_negative_number() {
        assert_eq!(tok("-3"), vec![Token::Number(-3.0)]);
    }

    #[test]
    fn test_symbol() {
        assert_eq!(tok("foo"), vec![Token::Symbol("foo".to_string())]);
    }

    #[test]
    fn test_operator_symbols() {
        assert_eq!(
            tok("add subtract multiply divide"),
            vec![
                Token::Symbol("add".to_string()),
                Token::Symbol("subtract".to_string()),
                Token::Symbol("multiply".to_string()),
                Token::Symbol("divide".to_string()),
            ]
        );
    }

    #[test]
    fn test_bool_true() {
        assert_eq!(tok("true"), vec![Token::Bool(true)]);
    }

    #[test]
    fn test_bool_false() {
        assert_eq!(tok("false"), vec![Token::Bool(false)]);
    }

    #[test]
    fn test_string() {
        assert_eq!(tok(r#""hello""#), vec![Token::Str("hello".to_string())]);
    }

    #[test]
    fn test_unterminated_string() {
        assert_eq!(tok_err(r#""oops"#), LexErrorKind::UnterminatedString);
    }

    #[test]
    fn test_unterminated_string_line() {
        let err = tokenize("foo\n\"oops").unwrap_err();
        assert_eq!(err.line, 2);
    }

    #[test]
    fn test_line_comment() {
        assert_eq!(tok("# comment\n42"), vec![Token::Number(42.0)]);
    }

    #[test]
    fn test_colon() {
        assert_eq!(
            tok("foo:"),
            vec![Token::Symbol("foo".to_string()), Token::Colon]
        );
    }

    #[test]
    fn test_tuple_tokens() {
        assert_eq!(
            tok("[1, 2]"),
            vec![
                Token::LBracket,
                Token::Number(1.0),
                Token::Comma,
                Token::Number(2.0),
                Token::RBracket,
            ]
        );
    }

    #[test]
    fn test_func_def_tokens() {
        assert_eq!(
            tok("double: (multiply [2, x])"),
            vec![
                Token::Symbol("double".to_string()),
                Token::Colon,
                Token::LParen,
                Token::Symbol("multiply".to_string()),
                Token::LBracket,
                Token::Number(2.0),
                Token::Comma,
                Token::Symbol("x".to_string()),
                Token::RBracket,
                Token::RParen,
            ]
        );
    }

    #[test]
    fn test_decimal_number() {
        assert_eq!(tok("3f14"), vec![Token::Number(3.14)]);
    }

    #[test]
    fn test_negative_decimal_number() {
        assert_eq!(tok("-1f5"), vec![Token::Number(-1.5)]);
    }

    #[test]
    fn test_line_numbers() {
        let tokens = tokenize("foo\nbar\nbaz").unwrap();
        let lines: Vec<usize> = tokens.iter().map(|(_, l)| *l).collect();
        assert_eq!(lines, vec![1, 2, 3]);
    }
}
