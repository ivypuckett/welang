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
    /// A `|` pipe operator appeared where no expression is valid.
    UnexpectedPipe,
    /// A pipe segment was empty (e.g. `(| f)` or `(f |)`).
    EmptyPipeSegment,
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
            ParseError::InvalidFuncDef => {
                write!(f, "invalid function definition: expected 'name: body'")
            }
            ParseError::UnexpectedTopLevel => write!(
                f,
                "unexpected expression at top level: only function definitions are allowed"
            ),
            ParseError::UnexpectedPipe => write!(f, "unexpected '|' outside of a pipe expression"),
            ParseError::EmptyPipeSegment => {
                write!(
                    f,
                    "empty pipe segment: '|' requires an expression on each side"
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

fn is_func_def_start(tokens: &[Token], pos: usize) -> bool {
    matches!(&tokens[pos], Token::Symbol(_))
        && pos + 1 < tokens.len()
        && tokens[pos + 1] == Token::Colon
}

/// Parse a source string into a list of top-level function definitions.
///
/// Only function definitions are allowed at the top level:
///
/// - `name: body`  — function with implicit parameter `x`
///
/// If the body does not reference `x`, the function behaves as a
/// zero-argument function. The body is exactly one expression (monadic).
/// Multi-arg functions do not exist; pass a tuple `[a, b]` to built-in
/// operators instead.
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

        // All functions have an implicit parameter `x`.
        let param_names: Vec<String> = vec!["x".to_string()];

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

            // Parse list contents, collecting pipe-separated segments.
            // `(a | b | c)` desugars to `(c (b a))`.
            let mut segments: Vec<Vec<Expr>> = vec![Vec::new()];
            loop {
                if *pos >= tokens.len() {
                    return Err(ParseError::UnmatchedOpenParen);
                }
                if tokens[*pos] == Token::RParen {
                    *pos += 1;
                    break;
                }
                if tokens[*pos] == Token::Pipe {
                    *pos += 1;
                    segments.push(Vec::new());
                    continue;
                }
                let expr = parse_expr(tokens, pos)?;
                segments.last_mut().unwrap().push(expr);
            }

            if segments.len() == 1 {
                // No pipe operator: return a normal list.
                Ok(Expr::List(segments.into_iter().next().unwrap()))
            } else {
                // Desugar pipeline left-to-right: (a | b | c) → (c (b a)).
                //
                // Each segment is folded right so its leftmost item is the
                // outermost (last-to-run) function and its rightmost item is
                // innermost (first-to-run):
                //
                //   (n3 n2 n1 x | n5 n4 | n6)
                //   → (n6 (n5 (n4 (n3 (n2 (n1 x))))))
                //
                // The first segment's rightmost item is used as-is (it should
                // be a value, e.g. `x` or a literal).  Every subsequent
                // segment's rightmost item is called with the accumulated value.
                let mut iter = segments.into_iter();
                let first = iter.next().unwrap();
                let mut acc = pipe_segment_first(first)?;
                for seg in iter {
                    acc = pipe_segment_rest(seg, acc)?;
                }
                Ok(acc)
            }
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
        Token::Pipe => Err(ParseError::UnexpectedPipe),

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

/// Build the first pipe segment by folding right.
///
/// The rightmost item is treated as a plain value (e.g. `x` or a literal).
/// Every item to its left is wrapped around it as a one-argument call:
///
/// ```text
/// [n3, n2, n1, x]  →  (n3 (n2 (n1 x)))
/// [double, x]      →  (double x)
/// [x]              →  x
/// ```
fn pipe_segment_first(items: Vec<Expr>) -> Result<Expr, ParseError> {
    match items.len() {
        0 => Err(ParseError::EmptyPipeSegment),
        1 => Ok(items.into_iter().next().unwrap()),
        _ => {
            let mut iter = items.into_iter().rev();
            let mut acc = iter.next().unwrap(); // rightmost = innermost value
            for func in iter {
                acc = Expr::List(vec![func, acc]);
            }
            Ok(acc)
        }
    }
}

/// Build a subsequent pipe segment, threading `arg` through it.
///
/// The rightmost item is called with `arg` as its input.  Every item to its
/// left wraps around the accumulated result:
///
/// ```text
/// [n5, n4], acc  →  (n5 (n4 acc))
/// [n6],     acc  →  (n6 acc)
/// ```
fn pipe_segment_rest(items: Vec<Expr>, arg: Expr) -> Result<Expr, ParseError> {
    match items.len() {
        0 => Err(ParseError::EmptyPipeSegment),
        1 => {
            let func = items.into_iter().next().unwrap();
            Ok(Expr::List(vec![func, arg]))
        }
        _ => {
            let mut iter = items.into_iter().rev();
            let last = iter.next().unwrap();
            let mut acc = Expr::List(vec![last, arg]); // rightmost receives arg
            for func in iter {
                acc = Expr::List(vec![func, acc]);
            }
            Ok(acc)
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
        assert_eq!(parse("f: 1 2").unwrap_err(), ParseError::UnexpectedTopLevel);
    }

    #[test]
    fn test_expr_after_func_def_is_error() {
        assert_eq!(
            parse("main: 0\n42").unwrap_err(),
            ParseError::UnexpectedTopLevel
        );
    }

    // ---- function defs -------------------------------------------------------

    #[test]
    fn test_func_def() {
        // `main: 0` — implicit param `x`, body doesn't reference it
        assert_eq!(
            parse("main: 0").unwrap(),
            vec![Expr::List(vec![
                Expr::Symbol("define".to_string()),
                Expr::List(vec![
                    Expr::Symbol("main".to_string()),
                    Expr::Symbol("x".to_string()),
                ]),
                Expr::Number(0.0),
            ])]
        );
    }

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
        let result = parse("foo: 1\nbar: (* [2, x])").unwrap();
        assert_eq!(result.len(), 2);
        // foo has implicit x but doesn't use it
        assert_eq!(
            result[0],
            Expr::List(vec![
                Expr::Symbol("define".to_string()),
                Expr::List(vec![
                    Expr::Symbol("foo".to_string()),
                    Expr::Symbol("x".to_string()),
                ]),
                Expr::Number(1.0),
            ])
        );
        // bar uses x
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
        // `foo:` at EOF — no body expression
        assert_eq!(parse("foo:").unwrap_err(), ParseError::InvalidFuncDef);
    }

    #[test]
    fn test_missing_body_before_next_def() {
        // `foo:` followed by another func def with no body for foo
        assert_eq!(
            parse("foo:\nbar: 1").unwrap_err(),
            ParseError::InvalidFuncDef
        );
    }

    #[test]
    fn test_unmatched_open_paren() {
        assert_eq!(
            parse("f: (+ 1 2").unwrap_err(),
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
        assert_eq!(parse("f: '").unwrap_err(), ParseError::MissingQuoteTarget);
    }

    #[test]
    fn test_lex_error_propagated() {
        assert!(matches!(
            parse(r#"f: "unterminated"#).unwrap_err(),
            ParseError::Lex(_)
        ));
    }

    #[test]
    fn test_comment_ignored() {
        assert_eq!(
            parse("; ignore\nmain: 0").unwrap(),
            vec![Expr::List(vec![
                Expr::Symbol("define".to_string()),
                Expr::List(vec![
                    Expr::Symbol("main".to_string()),
                    Expr::Symbol("x".to_string()),
                ]),
                Expr::Number(0.0),
            ])]
        );
    }

    // ---- pipe operator -------------------------------------------------------

    #[test]
    fn test_simple_pipe() {
        // `f: (x | double)` desugars to `f: (double x)`
        assert_eq!(
            parse("f: (x | double)").unwrap(),
            vec![Expr::List(vec![
                Expr::Symbol("define".to_string()),
                Expr::List(vec![
                    Expr::Symbol("f".to_string()),
                    Expr::Symbol("x".to_string()),
                ]),
                Expr::List(vec![
                    Expr::Symbol("double".to_string()),
                    Expr::Symbol("x".to_string()),
                ]),
            ])]
        );
    }

    #[test]
    fn test_chained_pipe() {
        // `f: (x | double | double)` desugars to `f: (double (double x))`
        assert_eq!(
            parse("f: (x | double | double)").unwrap(),
            vec![Expr::List(vec![
                Expr::Symbol("define".to_string()),
                Expr::List(vec![
                    Expr::Symbol("f".to_string()),
                    Expr::Symbol("x".to_string()),
                ]),
                Expr::List(vec![
                    Expr::Symbol("double".to_string()),
                    Expr::List(vec![
                        Expr::Symbol("double".to_string()),
                        Expr::Symbol("x".to_string()),
                    ]),
                ]),
            ])]
        );
    }

    #[test]
    fn test_pipe_with_call_on_left() {
        // `f: (double x | inc)` desugars to `f: (inc (double x))`
        assert_eq!(
            parse("f: (double x | inc)").unwrap(),
            vec![Expr::List(vec![
                Expr::Symbol("define".to_string()),
                Expr::List(vec![
                    Expr::Symbol("f".to_string()),
                    Expr::Symbol("x".to_string()),
                ]),
                Expr::List(vec![
                    Expr::Symbol("inc".to_string()),
                    Expr::List(vec![
                        Expr::Symbol("double".to_string()),
                        Expr::Symbol("x".to_string()),
                    ]),
                ]),
            ])]
        );
    }

    #[test]
    fn test_pipe_multi_element_segments() {
        // (n3 n2 n1 x | n5 n4) desugars to (n5 (n4 (n3 (n2 (n1 x)))))
        let sym = |s: &str| Expr::Symbol(s.to_string());
        let call = |f: Expr, a: Expr| Expr::List(vec![f, a]);
        let expected_body = call(
            sym("n5"),
            call(
                sym("n4"),
                call(sym("n3"), call(sym("n2"), call(sym("n1"), sym("x")))),
            ),
        );
        assert_eq!(
            parse("f: (n3 n2 n1 x | n5 n4)").unwrap(),
            vec![Expr::List(vec![
                sym("define"),
                Expr::List(vec![sym("f"), sym("x")]),
                expected_body,
            ])]
        );
    }

    #[test]
    fn test_pipe_empty_segment_error() {
        assert_eq!(
            parse("f: (x | | double)").unwrap_err(),
            ParseError::EmptyPipeSegment
        );
    }

    #[test]
    fn test_pipe_leading_pipe_error() {
        assert_eq!(
            parse("f: (| double)").unwrap_err(),
            ParseError::EmptyPipeSegment
        );
    }

    #[test]
    fn test_pipe_trailing_pipe_error() {
        assert_eq!(
            parse("f: (double |)").unwrap_err(),
            ParseError::EmptyPipeSegment
        );
    }
}
