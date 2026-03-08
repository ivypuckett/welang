use crate::lisp::lexer::{LexError, Token, tokenize};

/// An expression in the AST.
#[derive(Debug, PartialEq, Clone)]
pub enum Expr {
    Number(f64),
    Bool(bool),
    Str(String),
    Symbol(String),
    Quote(Box<Expr>),
    /// A parenthesised call or special form: `(f arg)`.
    List(Vec<Expr>),
    /// A tuple literal: `[e1, e2, ...]`.
    Tuple(Vec<Expr>),
    /// A rename-binding expression: `(y: body)` — binds `y = x` in `body`.
    Rename(String, Box<Expr>),
}

/// Errors that can occur during parsing.
#[derive(Debug, PartialEq)]
pub enum ParseError {
    Lex(LexError),
    UnmatchedOpenParen,
    UnmatchedOpenBracket,
    UnexpectedCloseParen,
    UnexpectedCloseBracket,
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
            ParseError::UnmatchedOpenBracket => write!(f, "unmatched '['"),
            ParseError::UnexpectedCloseParen => write!(f, "unexpected ')'"),
            ParseError::UnexpectedCloseBracket => write!(f, "unexpected ']'"),
            ParseError::MissingQuoteTarget => write!(f, "quote requires an expression"),
            ParseError::InvalidFuncDef => write!(
                f,
                "invalid function definition: expected 'name: () body' or 'name: body'"
            ),
            ParseError::UnexpectedTopLevel => write!(
                f,
                "unexpected expression at top level: only function definitions are allowed"
            ),
        }
    }
}

impl From<LexError> for ParseError {
    fn from(e: LexError) -> Self {
        ParseError::Lex(e)
    }
}

fn is_func_def_start(tokens: &[Token], pos: usize) -> bool {
    matches!(&tokens[pos], Token::Symbol(_))
        && pos + 1 < tokens.len()
        && tokens[pos + 1] == Token::Colon
}

/// Parse a source string into a list of top-level function definitions.
///
/// Only function definitions are allowed at the top level:
///
/// - `name: () body`  — zero-arg function
/// - `name: body`     — one-arg function; the input is implicitly bound to `x`
///
/// The body is exactly one expression (monadic). Multi-arg functions do not
/// exist; pass a tuple `[a, b]` to built-in operators instead.
pub fn parse(input: &str) -> Result<Vec<Expr>, ParseError> {
    let tokens = tokenize(input)?;
    let mut pos = 0;
    let mut exprs = Vec::new();

    while pos < tokens.len() {
        if !is_func_def_start(&tokens, pos) {
            return Err(ParseError::UnexpectedTopLevel);
        }

        let name = match &tokens[pos] {
            Token::Symbol(s) => s.clone(),
            _ => unreachable!(),
        };
        pos += 2; // consume name and colon

        // Determine arity.
        // `()` immediately after `:` → zero-arg.
        // Anything else → one-arg with implicit `x`.
        let param_names: Vec<String> = if pos + 1 < tokens.len()
            && tokens[pos] == Token::LParen
            && tokens[pos + 1] == Token::RParen
        {
            pos += 2; // consume `()`
            vec![]
        } else {
            vec!["x".to_string()]
        };

        // Parse exactly one body expression.
        if pos >= tokens.len() || is_func_def_start(&tokens, pos) {
            return Err(ParseError::InvalidFuncDef);
        }
        let body = parse_expr(&tokens, &mut pos)?;

        // After the body, must be another func def or EOF.
        if pos < tokens.len() && !is_func_def_start(&tokens, pos) {
            return Err(ParseError::UnexpectedTopLevel);
        }

        // Desugar to `(define (name params...) body)` for codegen.
        let mut sig = vec![Expr::Symbol(name)];
        sig.extend(param_names.into_iter().map(Expr::Symbol));
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

            // Check for rename syntax: `(name: body)`.
            if *pos < tokens.len()
                && matches!(&tokens[*pos], Token::Symbol(_))
                && *pos + 1 < tokens.len()
                && tokens[*pos + 1] == Token::Colon
            {
                let rename = match &tokens[*pos] {
                    Token::Symbol(s) => s.clone(),
                    _ => unreachable!(),
                };
                *pos += 2; // consume name and colon
                let body = parse_expr(tokens, pos)?;
                if *pos >= tokens.len() || tokens[*pos] != Token::RParen {
                    return Err(ParseError::UnmatchedOpenParen);
                }
                *pos += 1; // consume `)`
                return Ok(Expr::Rename(rename, Box::new(body)));
            }

            // Single-element `(expr)` — grouping, not a call.
            // Peek ahead: if the next token closes immediately after one expr,
            // we still parse it as a 1-element list and let codegen treat it
            // as grouping.
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

        Token::LBracket => {
            *pos += 1;
            let mut items = Vec::new();
            loop {
                if *pos >= tokens.len() {
                    return Err(ParseError::UnmatchedOpenBracket);
                }
                if tokens[*pos] == Token::RBracket {
                    *pos += 1;
                    break;
                }
                items.push(parse_expr(tokens, pos)?);
                // Skip optional comma separator.
                if *pos < tokens.len() && tokens[*pos] == Token::Comma {
                    *pos += 1;
                }
            }
            Ok(Expr::Tuple(items))
        }

        Token::RParen => Err(ParseError::UnexpectedCloseParen),
        Token::RBracket => Err(ParseError::UnexpectedCloseBracket),
        Token::Comma => Err(ParseError::UnexpectedCloseBracket), // misplaced comma
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

    // ---- top-level only accepts func defs ------------------------------------

    #[test]
    fn test_bare_number_is_error() {
        assert_eq!(parse("42").unwrap_err(), ParseError::UnexpectedTopLevel);
    }

    #[test]
    fn test_bare_expr_is_error() {
        assert_eq!(
            parse("(+ 1 2)").unwrap_err(),
            ParseError::UnexpectedTopLevel
        );
    }

    #[test]
    fn test_multi_body_is_error() {
        // After `1` the `2` is not a func def start.
        assert_eq!(
            parse("f: () 1 2").unwrap_err(),
            ParseError::UnexpectedTopLevel
        );
    }

    #[test]
    fn test_expr_after_func_def_is_error() {
        assert_eq!(
            parse("main: () 0\n42").unwrap_err(),
            ParseError::UnexpectedTopLevel
        );
    }

    // ---- zero-arg function defs ----------------------------------------------

    #[test]
    fn test_zero_arg_func() {
        assert_eq!(
            parse("main: () 0").unwrap(),
            vec![Expr::List(vec![
                Expr::Symbol("define".to_string()),
                Expr::List(vec![Expr::Symbol("main".to_string())]),
                Expr::Number(0.0),
            ])]
        );
    }

    // ---- one-arg function defs -----------------------------------------------

    #[test]
    fn test_one_arg_func() {
        // `double: (* [2, x])` — implicit param `x`
        assert_eq!(
            parse("double: (* [2, x])").unwrap(),
            vec![Expr::List(vec![
                Expr::Symbol("define".to_string()),
                Expr::List(vec![
                    Expr::Symbol("double".to_string()),
                    Expr::Symbol("x".to_string()),
                ]),
                Expr::List(vec![
                    Expr::Symbol("*".to_string()),
                    Expr::Tuple(vec![Expr::Number(2.0), Expr::Symbol("x".to_string())]),
                ]),
            ])]
        );
    }

    #[test]
    fn test_multiple_func_defs() {
        let result = parse("foo: () 1\nbar: (* [2, x])").unwrap();
        assert_eq!(result.len(), 2);
        // foo is zero-arg
        assert_eq!(
            result[0],
            Expr::List(vec![
                Expr::Symbol("define".to_string()),
                Expr::List(vec![Expr::Symbol("foo".to_string())]),
                Expr::Number(1.0),
            ])
        );
        // bar is one-arg
        assert_eq!(
            result[1],
            Expr::List(vec![
                Expr::Symbol("define".to_string()),
                Expr::List(vec![
                    Expr::Symbol("bar".to_string()),
                    Expr::Symbol("x".to_string()),
                ]),
                Expr::List(vec![
                    Expr::Symbol("*".to_string()),
                    Expr::Tuple(vec![Expr::Number(2.0), Expr::Symbol("x".to_string())]),
                ]),
            ])
        );
    }

    // ---- rename syntax -------------------------------------------------------

    #[test]
    fn test_rename_expr() {
        // `id: (n: n)` — rename x to n, return n
        assert_eq!(
            parse("id: (n: n)").unwrap(),
            vec![Expr::List(vec![
                Expr::Symbol("define".to_string()),
                Expr::List(vec![
                    Expr::Symbol("id".to_string()),
                    Expr::Symbol("x".to_string()),
                ]),
                Expr::Rename("n".to_string(), Box::new(Expr::Symbol("n".to_string()))),
            ])]
        );
    }

    // ---- tuples --------------------------------------------------------------

    #[test]
    fn test_tuple_parse() {
        // Inside a func body
        assert_eq!(
            parse("f: [1, 2]").unwrap(),
            vec![Expr::List(vec![
                Expr::Symbol("define".to_string()),
                Expr::List(vec![
                    Expr::Symbol("f".to_string()),
                    Expr::Symbol("x".to_string()),
                ]),
                Expr::Tuple(vec![Expr::Number(1.0), Expr::Number(2.0)]),
            ])]
        );
    }

    // ---- error cases ---------------------------------------------------------

    #[test]
    fn test_missing_body() {
        assert_eq!(parse("foo: ()").unwrap_err(), ParseError::InvalidFuncDef);
    }

    #[test]
    fn test_missing_body_one_arg() {
        // `foo:` followed by another func def with no body for foo
        assert_eq!(
            parse("foo:\nbar: () 1").unwrap_err(),
            ParseError::InvalidFuncDef
        );
    }

    #[test]
    fn test_unmatched_open_paren() {
        assert_eq!(
            parse("f: () (+ 1 2").unwrap_err(),
            ParseError::UnmatchedOpenParen
        );
    }

    #[test]
    fn test_unmatched_open_bracket() {
        assert_eq!(
            parse("f: [1, 2").unwrap_err(),
            ParseError::UnmatchedOpenBracket
        );
    }

    #[test]
    fn test_missing_quote_target() {
        assert_eq!(
            parse("f: () '").unwrap_err(),
            ParseError::MissingQuoteTarget
        );
    }

    #[test]
    fn test_lex_error_propagated() {
        assert!(matches!(
            parse(r#"f: () "unterminated"#).unwrap_err(),
            ParseError::Lex(_)
        ));
    }

    #[test]
    fn test_comment_ignored() {
        assert_eq!(
            parse("; ignore\nmain: () 0").unwrap(),
            vec![Expr::List(vec![
                Expr::Symbol("define".to_string()),
                Expr::List(vec![Expr::Symbol("main".to_string())]),
                Expr::Number(0.0),
            ])]
        );
    }
}
