use crate::lisp::lexer::{tokenize, LexError, Token};

/// An expression in the LISP AST.
#[derive(Debug, PartialEq, Clone)]
pub enum Expr {
    /// A numeric literal.
    Number(f64),
    /// A boolean literal (`#t` / `#f`).
    Bool(bool),
    /// A string literal.
    Str(String),
    /// A symbol (identifier or operator).
    Symbol(String),
    /// A quoted expression: `'expr` => `(quote expr)`.
    Quote(Box<Expr>),
    /// A parenthesised list of expressions.
    List(Vec<Expr>),
}

/// Errors that can occur during parsing.
#[derive(Debug, PartialEq)]
pub enum ParseError {
    /// A lexer error was encountered while tokenizing.
    Lex(LexError),
    /// A `(` was never closed.
    UnmatchedOpenParen,
    /// A `)` was found with no matching `(`.
    UnexpectedCloseParen,
    /// A `'` was not followed by an expression.
    MissingQuoteTarget,
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ParseError::Lex(e) => write!(f, "lex error: {e}"),
            ParseError::UnmatchedOpenParen => write!(f, "unmatched '('"),
            ParseError::UnexpectedCloseParen => write!(f, "unexpected ')'"),
            ParseError::MissingQuoteTarget => write!(f, "quote requires an expression"),
        }
    }
}

impl From<LexError> for ParseError {
    fn from(e: LexError) -> Self {
        ParseError::Lex(e)
    }
}

/// Parse a source string into a list of top-level expressions.
pub fn parse(input: &str) -> Result<Vec<Expr>, ParseError> {
    let tokens = tokenize(input)?;
    let mut pos = 0;
    let mut exprs = Vec::new();

    while pos < tokens.len() {
        let expr = parse_expr(&tokens, &mut pos)?;
        exprs.push(expr);
    }

    Ok(exprs)
}

/// Parse a single expression from `tokens` starting at `*pos`.
fn parse_expr(tokens: &[Token], pos: &mut usize) -> Result<Expr, ParseError> {
    if *pos >= tokens.len() {
        return Err(ParseError::UnmatchedOpenParen);
    }

    match &tokens[*pos] {
        Token::LParen => {
            *pos += 1;
            let mut list = Vec::new();
            loop {
                if *pos >= tokens.len() {
                    return Err(ParseError::UnmatchedOpenParen);
                }
                if tokens[*pos] == Token::RParen {
                    *pos += 1;
                    break;
                }
                list.push(parse_expr(tokens, pos)?);
            }
            Ok(Expr::List(list))
        }

        Token::RParen => Err(ParseError::UnexpectedCloseParen),

        Token::Quote => {
            *pos += 1;
            if *pos >= tokens.len() {
                return Err(ParseError::MissingQuoteTarget);
            }
            let inner = parse_expr(tokens, pos)?;
            Ok(Expr::Quote(Box::new(inner)))
        }

        Token::Number(n) => {
            let n = *n;
            *pos += 1;
            Ok(Expr::Number(n))
        }

        Token::Bool(b) => {
            let b = *b;
            *pos += 1;
            Ok(Expr::Bool(b))
        }

        Token::Str(s) => {
            let s = s.clone();
            *pos += 1;
            Ok(Expr::Str(s))
        }

        Token::Symbol(s) => {
            let s = s.clone();
            *pos += 1;
            Ok(Expr::Symbol(s))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---- atoms ----------------------------------------------------------------

    #[test]
    fn test_parse_number_integer() {
        assert_eq!(parse("42").unwrap(), vec![Expr::Number(42.0)]);
    }

    #[test]
    fn test_parse_number_float() {
        assert_eq!(parse("3.14").unwrap(), vec![Expr::Number(3.14)]);
    }

    #[test]
    fn test_parse_negative_number() {
        assert_eq!(parse("-7").unwrap(), vec![Expr::Number(-7.0)]);
    }

    #[test]
    fn test_parse_bool_true() {
        assert_eq!(parse("#t").unwrap(), vec![Expr::Bool(true)]);
    }

    #[test]
    fn test_parse_bool_false() {
        assert_eq!(parse("#f").unwrap(), vec![Expr::Bool(false)]);
    }

    #[test]
    fn test_parse_string() {
        assert_eq!(
            parse(r#""hello world""#).unwrap(),
            vec![Expr::Str("hello world".to_string())]
        );
    }

    #[test]
    fn test_parse_symbol() {
        assert_eq!(
            parse("foo").unwrap(),
            vec![Expr::Symbol("foo".to_string())]
        );
    }

    // ---- lists ----------------------------------------------------------------

    #[test]
    fn test_parse_empty_list() {
        assert_eq!(parse("()").unwrap(), vec![Expr::List(vec![])]);
    }

    #[test]
    fn test_parse_simple_call() {
        assert_eq!(
            parse("(+ 1 2)").unwrap(),
            vec![Expr::List(vec![
                Expr::Symbol("+".to_string()),
                Expr::Number(1.0),
                Expr::Number(2.0),
            ])]
        );
    }

    #[test]
    fn test_parse_nested_lists() {
        assert_eq!(
            parse("(+ (* 2 3) 4)").unwrap(),
            vec![Expr::List(vec![
                Expr::Symbol("+".to_string()),
                Expr::List(vec![
                    Expr::Symbol("*".to_string()),
                    Expr::Number(2.0),
                    Expr::Number(3.0),
                ]),
                Expr::Number(4.0),
            ])]
        );
    }

    #[test]
    fn test_parse_define() {
        assert_eq!(
            parse("(define x 10)").unwrap(),
            vec![Expr::List(vec![
                Expr::Symbol("define".to_string()),
                Expr::Symbol("x".to_string()),
                Expr::Number(10.0),
            ])]
        );
    }

    #[test]
    fn test_parse_lambda() {
        assert_eq!(
            parse("(lambda (x) x)").unwrap(),
            vec![Expr::List(vec![
                Expr::Symbol("lambda".to_string()),
                Expr::List(vec![Expr::Symbol("x".to_string())]),
                Expr::Symbol("x".to_string()),
            ])]
        );
    }

    #[test]
    fn test_parse_if() {
        assert_eq!(
            parse("(if #t 1 2)").unwrap(),
            vec![Expr::List(vec![
                Expr::Symbol("if".to_string()),
                Expr::Bool(true),
                Expr::Number(1.0),
                Expr::Number(2.0),
            ])]
        );
    }

    // ---- quote ----------------------------------------------------------------

    #[test]
    fn test_parse_quote_shorthand() {
        assert_eq!(
            parse("'x").unwrap(),
            vec![Expr::Quote(Box::new(Expr::Symbol("x".to_string())))]
        );
    }

    #[test]
    fn test_parse_quote_list() {
        assert_eq!(
            parse("'(1 2 3)").unwrap(),
            vec![Expr::Quote(Box::new(Expr::List(vec![
                Expr::Number(1.0),
                Expr::Number(2.0),
                Expr::Number(3.0),
            ])))]
        );
    }

    // ---- multiple top-level expressions ---------------------------------------

    #[test]
    fn test_parse_multiple_exprs() {
        assert_eq!(
            parse("(+ 1 2)\n(* 3 4)").unwrap(),
            vec![
                Expr::List(vec![
                    Expr::Symbol("+".to_string()),
                    Expr::Number(1.0),
                    Expr::Number(2.0),
                ]),
                Expr::List(vec![
                    Expr::Symbol("*".to_string()),
                    Expr::Number(3.0),
                    Expr::Number(4.0),
                ]),
            ]
        );
    }

    #[test]
    fn test_parse_with_comment() {
        assert_eq!(
            parse("; ignore this\n(+ 1 2)").unwrap(),
            vec![Expr::List(vec![
                Expr::Symbol("+".to_string()),
                Expr::Number(1.0),
                Expr::Number(2.0),
            ])]
        );
    }

    // ---- error cases ----------------------------------------------------------

    #[test]
    fn test_parse_unmatched_open_paren() {
        assert_eq!(parse("(+ 1 2").unwrap_err(), ParseError::UnmatchedOpenParen);
    }

    #[test]
    fn test_parse_unexpected_close_paren() {
        assert_eq!(
            parse(")").unwrap_err(),
            ParseError::UnexpectedCloseParen
        );
    }

    #[test]
    fn test_parse_missing_quote_target() {
        assert_eq!(parse("'").unwrap_err(), ParseError::MissingQuoteTarget);
    }

    #[test]
    fn test_parse_lex_error_propagated() {
        assert!(matches!(
            parse(r#""unterminated"#).unwrap_err(),
            ParseError::Lex(_)
        ));
    }
}
