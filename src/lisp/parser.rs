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
    /// A map literal: `{k1: v1, k2: v2, ...}`.
    Map(Vec<(String, Expr)>),
    /// A conditional expression: `{(cond1): v1, (cond2): v2, _: default}`.
    /// `None` in the key position represents the wildcard `_`.
    Cond(Vec<(Option<Expr>, Expr)>),
    /// A rename-binding expression: `(y: body)` — binds `y = x` in `body`.
    Rename(String, Box<Expr>),
}

/// The kind of error that occurred during parsing.
#[derive(Debug, PartialEq)]
pub enum ParseErrorKind {
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
    /// A `{` map or conditional literal was not closed.
    UnmatchedOpenBrace,
    /// A `}` appeared outside of a map or conditional literal.
    UnexpectedCloseBrace,
    /// A map entry was malformed (expected `key: value`).
    InvalidMapEntry,
    /// A conditional entry was malformed (expected `(expr): value` or `_: value`).
    InvalidCondEntry,
    /// A conditional expression is missing its required `_: value` wildcard.
    MissingCondWildcard,
    /// The `_` wildcard appeared before the last entry in a conditional.
    MisplacedCondWildcard,
}

impl std::fmt::Display for ParseErrorKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ParseErrorKind::Lex(e) => write!(f, "lex error: {e}"),
            ParseErrorKind::UnmatchedOpenParen => write!(f, "unmatched '('"),
            ParseErrorKind::UnmatchedOpenBracket => write!(f, "unmatched '['"),
            ParseErrorKind::UnexpectedCloseParen => write!(f, "unexpected ')'"),
            ParseErrorKind::UnexpectedCloseBracket => write!(f, "unexpected ']'"),
            ParseErrorKind::MissingQuoteTarget => write!(f, "quote requires an expression"),
            ParseErrorKind::InvalidFuncDef => {
                write!(f, "invalid function definition: expected 'name: body'")
            }
            ParseErrorKind::UnexpectedTopLevel => write!(
                f,
                "unexpected expression at top level: only function definitions are allowed"
            ),
            ParseErrorKind::UnexpectedPipe => {
                write!(f, "unexpected '|' outside of a pipe expression")
            }
            ParseErrorKind::EmptyPipeSegment => {
                write!(
                    f,
                    "empty pipe segment: '|' requires an expression on each side"
                )
            }
            ParseErrorKind::UnmatchedOpenBrace => write!(f, "unmatched '{{'"),
            ParseErrorKind::UnexpectedCloseBrace => write!(f, "unexpected '}}'"),
            ParseErrorKind::InvalidMapEntry => {
                write!(f, "invalid map entry: expected 'key: value'")
            }
            ParseErrorKind::InvalidCondEntry => write!(
                f,
                "invalid conditional entry: expected '(condition): value' or '_: value'"
            ),
            ParseErrorKind::MissingCondWildcard => write!(
                f,
                "conditional expression requires a '_: value' wildcard as the last entry"
            ),
            ParseErrorKind::MisplacedCondWildcard => write!(
                f,
                "the '_' wildcard must be the last entry in a conditional"
            ),
        }
    }
}

/// A parse error with the 1-indexed source line where it occurred.
#[derive(Debug, PartialEq)]
pub struct ParseError {
    pub kind: ParseErrorKind,
    pub line: usize,
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.kind)
    }
}

impl From<LexError> for ParseError {
    fn from(e: LexError) -> Self {
        ParseError {
            line: e.line,
            kind: ParseErrorKind::Lex(e),
        }
    }
}

/// Return the line number of `tokens[pos]`, or the last token's line if past
/// the end, or 1 if the token list is empty.
fn line_at(tokens: &[(Token, usize)], pos: usize) -> usize {
    if pos < tokens.len() {
        tokens[pos].1
    } else {
        tokens.last().map(|(_, l)| *l).unwrap_or(1)
    }
}

fn is_func_def_start(tokens: &[(Token, usize)], pos: usize) -> bool {
    matches!(&tokens[pos].0, Token::Symbol(_))
        && pos + 1 < tokens.len()
        && tokens[pos + 1].0 == Token::Colon
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
            return Err(ParseError {
                kind: ParseErrorKind::UnexpectedTopLevel,
                line: tokens[pos].1,
            });
        }

        let name = match &tokens[pos].0 {
            Token::Symbol(s) => s.clone(),
            _ => unreachable!(),
        };
        let def_line = tokens[pos].1;
        pos += 2; // consume name and colon

        // All functions have an implicit parameter `x`.
        let param_names: Vec<String> = vec!["x".to_string()];

        // Parse exactly one body expression.
        if pos >= tokens.len() || is_func_def_start(&tokens, pos) {
            return Err(ParseError {
                kind: ParseErrorKind::InvalidFuncDef,
                line: def_line,
            });
        }
        let body = parse_expr(&tokens, &mut pos)?;

        // After the body, must be another func def or EOF.
        if pos < tokens.len() && !is_func_def_start(&tokens, pos) {
            return Err(ParseError {
                kind: ParseErrorKind::UnexpectedTopLevel,
                line: tokens[pos].1,
            });
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
fn parse_expr(tokens: &[(Token, usize)], pos: &mut usize) -> Result<Expr, ParseError> {
    if *pos >= tokens.len() {
        return Err(ParseError {
            kind: ParseErrorKind::UnmatchedOpenParen,
            line: line_at(tokens, *pos),
        });
    }

    let tok_line = tokens[*pos].1;

    match tokens[*pos].0.clone() {
        Token::LParen => {
            *pos += 1;

            // Check for rename syntax: `(name: body)`.
            if *pos < tokens.len()
                && matches!(&tokens[*pos].0, Token::Symbol(_))
                && *pos + 1 < tokens.len()
                && tokens[*pos + 1].0 == Token::Colon
            {
                let rename = match &tokens[*pos].0 {
                    Token::Symbol(s) => s.clone(),
                    _ => unreachable!(),
                };
                *pos += 2; // consume name and colon
                let body = parse_expr(tokens, pos)?;
                if *pos >= tokens.len() || tokens[*pos].0 != Token::RParen {
                    return Err(ParseError {
                        kind: ParseErrorKind::UnmatchedOpenParen,
                        line: tok_line,
                    });
                }
                *pos += 1; // consume `)`
                return Ok(Expr::Rename(rename, Box::new(body)));
            }

            // Parse list contents, collecting pipe-separated segments.
            // `(a | b | c)` desugars to `(c (b a))`.
            let mut segments: Vec<Vec<Expr>> = vec![Vec::new()];
            loop {
                if *pos >= tokens.len() {
                    return Err(ParseError {
                        kind: ParseErrorKind::UnmatchedOpenParen,
                        line: tok_line,
                    });
                }
                if tokens[*pos].0 == Token::RParen {
                    *pos += 1;
                    break;
                }
                if tokens[*pos].0 == Token::Pipe {
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
                let mut acc = pipe_segment_first(first).map_err(|kind| ParseError {
                    kind,
                    line: tok_line,
                })?;
                for seg in iter {
                    acc = pipe_segment_rest(seg, acc).map_err(|kind| ParseError {
                        kind,
                        line: tok_line,
                    })?;
                }
                Ok(acc)
            }
        }

        Token::LBracket => {
            *pos += 1;
            let mut items = Vec::new();
            loop {
                if *pos >= tokens.len() {
                    return Err(ParseError {
                        kind: ParseErrorKind::UnmatchedOpenBracket,
                        line: tok_line,
                    });
                }
                if tokens[*pos].0 == Token::RBracket {
                    *pos += 1;
                    break;
                }
                items.push(parse_expr(tokens, pos)?);
                // Skip optional comma separator.
                if *pos < tokens.len() && tokens[*pos].0 == Token::Comma {
                    *pos += 1;
                }
            }
            Ok(Expr::Tuple(items))
        }

        Token::LBrace => {
            *pos += 1;

            // Determine whether this is a conditional `{(cond): v, ..., _: v}`
            // or a data map `{key: v, ...}`.  A leading `(` or `_` signals Cond.
            let is_cond = *pos < tokens.len()
                && match &tokens[*pos].0 {
                    Token::LParen => true,
                    Token::Symbol(s) if s == "_" => true,
                    _ => false,
                };

            if is_cond {
                let mut entries: Vec<(Option<Expr>, Expr)> = Vec::new();
                let mut has_wildcard = false;
                loop {
                    if *pos >= tokens.len() {
                        return Err(ParseError {
                            kind: ParseErrorKind::UnmatchedOpenBrace,
                            line: tok_line,
                        });
                    }
                    if tokens[*pos].0 == Token::RBrace {
                        *pos += 1;
                        break;
                    }
                    if has_wildcard {
                        return Err(ParseError {
                            kind: ParseErrorKind::MisplacedCondWildcard,
                            line: tokens[*pos].1,
                        });
                    }
                    // Parse key: `(expr)` or `_`.
                    let key = if tokens[*pos].0 == Token::LParen {
                        Some(parse_expr(tokens, pos)?)
                    } else if matches!(&tokens[*pos].0, Token::Symbol(s) if s == "_") {
                        *pos += 1;
                        has_wildcard = true;
                        None
                    } else {
                        return Err(ParseError {
                            kind: ParseErrorKind::InvalidCondEntry,
                            line: tokens[*pos].1,
                        });
                    };
                    if *pos >= tokens.len() || tokens[*pos].0 != Token::Colon {
                        return Err(ParseError {
                            kind: ParseErrorKind::InvalidCondEntry,
                            line: line_at(tokens, *pos),
                        });
                    }
                    *pos += 1; // consume colon
                    let val = parse_expr(tokens, pos)?;
                    entries.push((key, val));
                    // Skip optional comma separator.
                    if *pos < tokens.len() && tokens[*pos].0 == Token::Comma {
                        *pos += 1;
                    }
                }
                if !has_wildcard {
                    return Err(ParseError {
                        kind: ParseErrorKind::MissingCondWildcard,
                        line: tok_line,
                    });
                }
                Ok(Expr::Cond(entries))
            } else {
                let mut entries: Vec<(String, Expr)> = Vec::new();
                loop {
                    if *pos >= tokens.len() {
                        return Err(ParseError {
                            kind: ParseErrorKind::UnmatchedOpenBrace,
                            line: tok_line,
                        });
                    }
                    if tokens[*pos].0 == Token::RBrace {
                        *pos += 1;
                        break;
                    }
                    // Expect: Symbol Colon Expr
                    let key = match &tokens[*pos].0 {
                        Token::Symbol(s) => s.clone(),
                        _ => {
                            return Err(ParseError {
                                kind: ParseErrorKind::InvalidMapEntry,
                                line: tokens[*pos].1,
                            });
                        }
                    };
                    *pos += 1; // consume key
                    if *pos >= tokens.len() || tokens[*pos].0 != Token::Colon {
                        return Err(ParseError {
                            kind: ParseErrorKind::InvalidMapEntry,
                            line: line_at(tokens, *pos),
                        });
                    }
                    *pos += 1; // consume colon
                    let val = parse_expr(tokens, pos)?;
                    entries.push((key, val));
                    // Skip optional comma separator.
                    if *pos < tokens.len() && tokens[*pos].0 == Token::Comma {
                        *pos += 1;
                    }
                }
                Ok(Expr::Map(entries))
            }
        }

        Token::RParen => Err(ParseError {
            kind: ParseErrorKind::UnexpectedCloseParen,
            line: tok_line,
        }),
        Token::RBracket => Err(ParseError {
            kind: ParseErrorKind::UnexpectedCloseBracket,
            line: tok_line,
        }),
        Token::RBrace => Err(ParseError {
            kind: ParseErrorKind::UnexpectedCloseBrace,
            line: tok_line,
        }),
        Token::Comma => Err(ParseError {
            kind: ParseErrorKind::UnexpectedCloseBracket, // misplaced comma
            line: tok_line,
        }),
        Token::Colon => Err(ParseError {
            kind: ParseErrorKind::InvalidFuncDef,
            line: tok_line,
        }),
        Token::Pipe => Err(ParseError {
            kind: ParseErrorKind::UnexpectedPipe,
            line: tok_line,
        }),

        Token::Quote => {
            *pos += 1;
            if *pos >= tokens.len() {
                return Err(ParseError {
                    kind: ParseErrorKind::MissingQuoteTarget,
                    line: tok_line,
                });
            }
            let inner = parse_expr(tokens, pos)?;
            Ok(Expr::Quote(Box::new(inner)))
        }

        Token::Number(n) => {
            *pos += 1;
            Ok(Expr::Number(n))
        }

        Token::Bool(b) => {
            *pos += 1;
            Ok(Expr::Bool(b))
        }

        Token::Str(s) => {
            *pos += 1;
            Ok(Expr::Str(s))
        }

        Token::Symbol(s) => {
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
fn pipe_segment_first(items: Vec<Expr>) -> Result<Expr, ParseErrorKind> {
    match items.len() {
        0 => Err(ParseErrorKind::EmptyPipeSegment),
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
fn pipe_segment_rest(items: Vec<Expr>, arg: Expr) -> Result<Expr, ParseErrorKind> {
    match items.len() {
        0 => Err(ParseErrorKind::EmptyPipeSegment),
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

    /// Unwrap the error kind from a parse result for concise assertions.
    fn parse_err_kind(input: &str) -> ParseErrorKind {
        parse(input).unwrap_err().kind
    }

    // ---- top-level only accepts func defs ------------------------------------

    #[test]
    fn test_bare_number_is_error() {
        assert_eq!(parse_err_kind("42"), ParseErrorKind::UnexpectedTopLevel);
    }

    #[test]
    fn test_bare_expr_is_error() {
        assert_eq!(
            parse_err_kind("(+ 1 2)"),
            ParseErrorKind::UnexpectedTopLevel
        );
    }

    #[test]
    fn test_multi_body_is_error() {
        // After `1` the `2` is not a func def start.
        assert_eq!(parse_err_kind("f: 1 2"), ParseErrorKind::UnexpectedTopLevel);
    }

    #[test]
    fn test_expr_after_func_def_is_error() {
        assert_eq!(
            parse_err_kind("main: 0\n42"),
            ParseErrorKind::UnexpectedTopLevel
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
        assert_eq!(parse_err_kind("foo:"), ParseErrorKind::InvalidFuncDef);
    }

    #[test]
    fn test_missing_body_before_next_def() {
        // `foo:` followed by another func def with no body for foo
        assert_eq!(
            parse_err_kind("foo:\nbar: 1"),
            ParseErrorKind::InvalidFuncDef
        );
    }

    #[test]
    fn test_unmatched_open_paren() {
        assert_eq!(
            parse_err_kind("f: (+ 1 2"),
            ParseErrorKind::UnmatchedOpenParen
        );
    }

    #[test]
    fn test_unmatched_open_bracket() {
        assert_eq!(
            parse_err_kind("f: [1, 2"),
            ParseErrorKind::UnmatchedOpenBracket
        );
    }

    #[test]
    fn test_missing_quote_target() {
        assert_eq!(parse_err_kind("f: '"), ParseErrorKind::MissingQuoteTarget);
    }

    #[test]
    fn test_lex_error_propagated() {
        assert!(matches!(
            parse_err_kind(r#"f: "unterminated"#),
            ParseErrorKind::Lex(_)
        ));
    }

    #[test]
    fn test_comment_ignored() {
        assert_eq!(
            parse("# ignore\nmain: 0").unwrap(),
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

    // ---- line number reporting -----------------------------------------------

    #[test]
    fn test_error_line_number_first_line() {
        let err = parse("42").unwrap_err();
        assert_eq!(err.line, 1);
    }

    #[test]
    fn test_error_line_number_second_line() {
        let err = parse("main: 0\n42").unwrap_err();
        assert_eq!(err.line, 2);
    }

    #[test]
    fn test_error_line_unmatched_paren() {
        // The `(` is on line 2.
        let err = parse("main: 0\nf: (+ 1 2").unwrap_err();
        assert_eq!(err.line, 2);
        assert_eq!(err.kind, ParseErrorKind::UnmatchedOpenParen);
    }

    #[test]
    fn test_lex_error_line_propagated() {
        // The unterminated string starts on line 3.
        let err = parse("a: 1\nb: 2\nc: \"oops").unwrap_err();
        assert_eq!(err.line, 3);
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
            parse_err_kind("f: (x | | double)"),
            ParseErrorKind::EmptyPipeSegment
        );
    }

    #[test]
    fn test_pipe_leading_pipe_error() {
        assert_eq!(
            parse_err_kind("f: (| double)"),
            ParseErrorKind::EmptyPipeSegment
        );
    }

    #[test]
    fn test_pipe_trailing_pipe_error() {
        assert_eq!(
            parse_err_kind("f: (double |)"),
            ParseErrorKind::EmptyPipeSegment
        );
    }

    // ---- decimal number syntax (3f14) ----------------------------------------

    #[test]
    fn test_decimal_literal_in_func_body() {
        // `3f14` in source should parse to Expr::Number(3.14).
        let result = parse("f: 3f14").unwrap();
        assert_eq!(
            result[0],
            Expr::List(vec![
                Expr::Symbol("define".to_string()),
                Expr::List(vec![
                    Expr::Symbol("f".to_string()),
                    Expr::Symbol("x".to_string()),
                ]),
                Expr::Number(3.14),
            ])
        );
    }

    #[test]
    fn test_negative_decimal_literal_in_func_body() {
        // `-1f5` in source should parse to Expr::Number(-1.5).
        let result = parse("f: -1f5").unwrap();
        assert_eq!(
            result[0],
            Expr::List(vec![
                Expr::Symbol("define".to_string()),
                Expr::List(vec![
                    Expr::Symbol("f".to_string()),
                    Expr::Symbol("x".to_string()),
                ]),
                Expr::Number(-1.5),
            ])
        );
    }

    #[test]
    fn test_decimal_in_tuple() {
        // Decimal numbers should be accepted inside tuple expressions.
        let result = parse("f: [1f5, 2f5]").unwrap();
        assert_eq!(
            result[0],
            Expr::List(vec![
                Expr::Symbol("define".to_string()),
                Expr::List(vec![
                    Expr::Symbol("f".to_string()),
                    Expr::Symbol("x".to_string()),
                ]),
                Expr::Tuple(vec![Expr::Number(1.5), Expr::Number(2.5)]),
            ])
        );
    }

    // ---- conditional expressions ---------------------------------------------

    #[test]
    fn test_cond_simple() {
        // `f: {(x): 1, _: 0}`
        let sym = |s: &str| Expr::Symbol(s.to_string());
        let result = parse("f: {(x): 1, _: 0}").unwrap();
        assert_eq!(
            result[0],
            Expr::List(vec![
                sym("define"),
                Expr::List(vec![sym("f"), sym("x")]),
                Expr::Cond(vec![
                    (Some(Expr::List(vec![sym("x")])), Expr::Number(1.0)),
                    (None, Expr::Number(0.0)),
                ]),
            ])
        );
    }

    #[test]
    fn test_cond_multi_arm() {
        // `f: {(lessThan [x, 0]): 1, (greaterThan [x, 0]): 2, _: 0}`
        let sym = |s: &str| Expr::Symbol(s.to_string());
        let result = parse("f: {(lessThan [x, 0]): 1, (greaterThan [x, 0]): 2, _: 0}").unwrap();
        assert_eq!(
            result[0],
            Expr::List(vec![
                sym("define"),
                Expr::List(vec![sym("f"), sym("x")]),
                Expr::Cond(vec![
                    (
                        Some(Expr::List(vec![
                            sym("lessThan"),
                            Expr::Tuple(vec![sym("x"), Expr::Number(0.0)]),
                        ])),
                        Expr::Number(1.0),
                    ),
                    (
                        Some(Expr::List(vec![
                            sym("greaterThan"),
                            Expr::Tuple(vec![sym("x"), Expr::Number(0.0)]),
                        ])),
                        Expr::Number(2.0),
                    ),
                    (None, Expr::Number(0.0)),
                ]),
            ])
        );
    }

    #[test]
    fn test_cond_missing_wildcard_error() {
        assert_eq!(
            parse_err_kind("f: {(x): 1}"),
            ParseErrorKind::MissingCondWildcard
        );
    }

    #[test]
    fn test_cond_misplaced_wildcard_error() {
        assert_eq!(
            parse_err_kind("f: {_: 0, (x): 1}"),
            ParseErrorKind::MisplacedCondWildcard
        );
    }

    #[test]
    fn test_cond_wildcard_only() {
        // `{_: 42}` — only a wildcard, always returns 42
        let sym = |s: &str| Expr::Symbol(s.to_string());
        let result = parse("f: {_: 42}").unwrap();
        assert_eq!(
            result[0],
            Expr::List(vec![
                sym("define"),
                Expr::List(vec![sym("f"), sym("x")]),
                Expr::Cond(vec![(None, Expr::Number(42.0))]),
            ])
        );
    }
}
