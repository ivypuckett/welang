use crate::lisp::lexer::{LexError, Token, tokenize};

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
    /// A `name:` function definition was malformed.
    InvalidFuncDef,
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ParseError::Lex(e) => write!(f, "lex error: {e}"),
            ParseError::UnmatchedOpenParen => write!(f, "unmatched '('"),
            ParseError::UnexpectedCloseParen => write!(f, "unexpected ')'"),
            ParseError::MissingQuoteTarget => write!(f, "quote requires an expression"),
            ParseError::InvalidFuncDef => {
                write!(
                    f,
                    "invalid function definition: expected 'name: (params) body'"
                )
            }
        }
    }
}

impl From<LexError> for ParseError {
    fn from(e: LexError) -> Self {
        ParseError::Lex(e)
    }
}

/// Parse a source string into a list of top-level expressions.
///
/// Function definitions use the syntax `name: (params...) body...` and are
/// desugared into `(define (name params...) body...)` during parsing.
/// Global constants still use `(define NAME value)`.
pub fn parse(input: &str) -> Result<Vec<Expr>, ParseError> {
    let tokens = tokenize(input)?;
    let mut pos = 0;
    let mut exprs = Vec::new();

    while pos < tokens.len() {
        // New function definition syntax: name: (params...) body...
        if let Token::Symbol(name) = &tokens[pos]
            && pos + 1 < tokens.len()
            && tokens[pos + 1] == Token::Colon
        {
            let name = name.clone();
            pos += 2; // consume name and colon

            // Next must be the parameter list
            if pos >= tokens.len() || tokens[pos] != Token::LParen {
                return Err(ParseError::InvalidFuncDef);
            }
            let params_expr = parse_expr(&tokens, &mut pos)?;
            let param_exprs = match params_expr {
                Expr::List(items) => items,
                _ => return Err(ParseError::InvalidFuncDef),
            };

            // Collect body expressions until the next function definition or end of input.
            let mut body = Vec::new();
            while pos < tokens.len() {
                // Look ahead: Symbol followed by Colon signals a new function definition.
                if matches!(&tokens[pos], Token::Symbol(_))
                    && pos + 1 < tokens.len()
                    && tokens[pos + 1] == Token::Colon
                {
                    break;
                }
                body.push(parse_expr(&tokens, &mut pos)?);
            }

            // Desugar to (define (name params...) body...)
            let mut sig = vec![Expr::Symbol(name)];
            sig.extend(param_exprs);
            let mut items = vec![Expr::Symbol("define".to_string()), Expr::List(sig)];
            items.extend(body);
            exprs.push(Expr::List(items));
            continue;
        }

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

        Token::Colon => Err(ParseError::InvalidFuncDef),

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
        assert_eq!(parse("foo").unwrap(), vec![Expr::Symbol("foo".to_string())]);
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
        // (define NAME value) is still the global constant syntax
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
    fn test_parse_func_def_no_params() {
        assert_eq!(
            parse("main: () 0").unwrap(),
            vec![Expr::List(vec![
                Expr::Symbol("define".to_string()),
                Expr::List(vec![Expr::Symbol("main".to_string())]),
                Expr::Number(0.0),
            ])]
        );
    }

    #[test]
    fn test_parse_func_def_with_params() {
        assert_eq!(
            parse("add: (a b) (+ a b)").unwrap(),
            vec![Expr::List(vec![
                Expr::Symbol("define".to_string()),
                Expr::List(vec![
                    Expr::Symbol("add".to_string()),
                    Expr::Symbol("a".to_string()),
                    Expr::Symbol("b".to_string()),
                ]),
                Expr::List(vec![
                    Expr::Symbol("+".to_string()),
                    Expr::Symbol("a".to_string()),
                    Expr::Symbol("b".to_string()),
                ]),
            ])]
        );
    }

    #[test]
    fn test_parse_func_def_multi_body() {
        assert_eq!(
            parse("f: () 1 2").unwrap(),
            vec![Expr::List(vec![
                Expr::Symbol("define".to_string()),
                Expr::List(vec![Expr::Symbol("f".to_string())]),
                Expr::Number(1.0),
                Expr::Number(2.0),
            ])]
        );
    }

    #[test]
    fn test_parse_multiple_func_defs() {
        let result = parse("foo: () 1\nbar: () 2").unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(
            result[0],
            Expr::List(vec![
                Expr::Symbol("define".to_string()),
                Expr::List(vec![Expr::Symbol("foo".to_string())]),
                Expr::Number(1.0),
            ])
        );
        assert_eq!(
            result[1],
            Expr::List(vec![
                Expr::Symbol("define".to_string()),
                Expr::List(vec![Expr::Symbol("bar".to_string())]),
                Expr::Number(2.0),
            ])
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
        assert_eq!(parse(")").unwrap_err(), ParseError::UnexpectedCloseParen);
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

    #[test]
    fn test_parse_invalid_func_def_missing_params() {
        assert_eq!(parse("foo:").unwrap_err(), ParseError::InvalidFuncDef);
    }
}
