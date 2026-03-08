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
    /// A top-level expression appeared outside of a function definition.
    UnexpectedTopLevel,
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
            ParseError::UnexpectedTopLevel => {
                write!(
                    f,
                    "unexpected expression at top level: only function definitions are allowed"
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

/// Returns true if the token stream at `pos` starts a new function definition.
fn is_func_def_start(tokens: &[Token], pos: usize) -> bool {
    matches!(&tokens[pos], Token::Symbol(_))
        && pos + 1 < tokens.len()
        && tokens[pos + 1] == Token::Colon
}

/// Parse a source string into a list of top-level function definitions.
///
/// Only function definitions are allowed at the top level:
///   `name: (params...) body`
///
/// The body is a single expression (monadic). Any token that cannot start a
/// new function definition after the body is a compile-time error.
pub fn parse(input: &str) -> Result<Vec<Expr>, ParseError> {
    let tokens = tokenize(input)?;
    let mut pos = 0;
    let mut exprs = Vec::new();

    while pos < tokens.len() {
        // Top level must always be a function definition.
        if !is_func_def_start(&tokens, pos) {
            return Err(ParseError::UnexpectedTopLevel);
        }

        let name = match &tokens[pos] {
            Token::Symbol(s) => s.clone(),
            _ => unreachable!(),
        };
        pos += 2; // consume name and colon

        // Next must be the parameter list.
        if pos >= tokens.len() || tokens[pos] != Token::LParen {
            return Err(ParseError::InvalidFuncDef);
        }
        let params_expr = parse_expr(&tokens, &mut pos)?;
        let param_exprs = match params_expr {
            Expr::List(items) => items,
            _ => return Err(ParseError::InvalidFuncDef),
        };

        // Body is exactly one expression.
        if pos >= tokens.len() || is_func_def_start(&tokens, pos) {
            return Err(ParseError::InvalidFuncDef);
        }
        let body = parse_expr(&tokens, &mut pos)?;

        // After the body, the next token (if any) must start another func def.
        if pos < tokens.len() && !is_func_def_start(&tokens, pos) {
            return Err(ParseError::UnexpectedTopLevel);
        }

        // Desugar to (define (name params...) body) for codegen.
        let mut sig = vec![Expr::Symbol(name)];
        sig.extend(param_exprs);
        let items = vec![Expr::Symbol("define".to_string()), Expr::List(sig), body];
        exprs.push(Expr::List(items));
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
        // Numbers are only valid inside function bodies; bare numbers at
        // top level are rejected.
        assert_eq!(parse("42").unwrap_err(), ParseError::UnexpectedTopLevel);
    }

    #[test]
    fn test_parse_symbol() {
        assert_eq!(parse("foo").unwrap_err(), ParseError::UnexpectedTopLevel);
    }

    // ---- func defs ------------------------------------------------------------

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
        // (lambda ...) is a valid single-expression body
        assert_eq!(
            parse("f: (x) (lambda (y) (+ x y))").unwrap(),
            vec![Expr::List(vec![
                Expr::Symbol("define".to_string()),
                Expr::List(vec![
                    Expr::Symbol("f".to_string()),
                    Expr::Symbol("x".to_string()),
                ]),
                Expr::List(vec![
                    Expr::Symbol("lambda".to_string()),
                    Expr::List(vec![Expr::Symbol("y".to_string())]),
                    Expr::List(vec![
                        Expr::Symbol("+".to_string()),
                        Expr::Symbol("x".to_string()),
                        Expr::Symbol("y".to_string()),
                    ]),
                ]),
            ])]
        );
    }

    #[test]
    fn test_parse_if() {
        assert_eq!(
            parse("f: () (if #t 1 2)").unwrap(),
            vec![Expr::List(vec![
                Expr::Symbol("define".to_string()),
                Expr::List(vec![Expr::Symbol("f".to_string())]),
                Expr::List(vec![
                    Expr::Symbol("if".to_string()),
                    Expr::Bool(true),
                    Expr::Number(1.0),
                    Expr::Number(2.0),
                ]),
            ])]
        );
    }

    // ---- monadic body enforcement --------------------------------------------

    #[test]
    fn test_parse_func_def_multi_body_is_error() {
        // The second expression is orphaned at the top level.
        assert_eq!(
            parse("f: () 1 2").unwrap_err(),
            ParseError::UnexpectedTopLevel
        );
    }

    #[test]
    fn test_parse_standalone_expr_is_error() {
        assert_eq!(
            parse("(+ 1 2)").unwrap_err(),
            ParseError::UnexpectedTopLevel
        );
    }

    #[test]
    fn test_parse_define_at_top_level_is_error() {
        // define is no longer a valid user-facing keyword.
        assert_eq!(
            parse("(define x 10)").unwrap_err(),
            ParseError::UnexpectedTopLevel
        );
    }

    #[test]
    fn test_parse_expr_after_func_def_is_error() {
        // The `42` after `main: () 0` is a standalone expression.
        assert_eq!(
            parse("main: () 0\n42").unwrap_err(),
            ParseError::UnexpectedTopLevel
        );
    }

    // ---- quote ----------------------------------------------------------------

    #[test]
    fn test_parse_quote_shorthand() {
        assert_eq!(
            parse("f: () 'x").unwrap(),
            vec![Expr::List(vec![
                Expr::Symbol("define".to_string()),
                Expr::List(vec![Expr::Symbol("f".to_string())]),
                Expr::Quote(Box::new(Expr::Symbol("x".to_string()))),
            ])]
        );
    }

    // ---- multiple top-level expressions ---------------------------------------

    #[test]
    fn test_parse_with_comment() {
        assert_eq!(
            parse("; ignore this\nmain: () 0").unwrap(),
            vec![Expr::List(vec![
                Expr::Symbol("define".to_string()),
                Expr::List(vec![Expr::Symbol("main".to_string())]),
                Expr::Number(0.0),
            ])]
        );
    }

    // ---- error cases ----------------------------------------------------------

    #[test]
    fn test_parse_unmatched_open_paren() {
        assert_eq!(
            parse("f: () (+ 1 2").unwrap_err(),
            ParseError::UnmatchedOpenParen
        );
    }

    #[test]
    fn test_parse_missing_quote_target() {
        assert_eq!(
            parse("f: () '").unwrap_err(),
            ParseError::MissingQuoteTarget
        );
    }

    #[test]
    fn test_parse_lex_error_propagated() {
        assert!(matches!(
            parse(r#"f: () "unterminated"#).unwrap_err(),
            ParseError::Lex(_)
        ));
    }

    #[test]
    fn test_parse_invalid_func_def_missing_params() {
        assert_eq!(parse("foo:").unwrap_err(), ParseError::InvalidFuncDef);
    }

    #[test]
    fn test_parse_invalid_func_def_missing_body() {
        assert_eq!(
            parse("foo: ()\nbar: () 1").unwrap_err(),
            ParseError::InvalidFuncDef
        );
    }
}
