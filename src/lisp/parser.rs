use crate::lisp::lexer::{LexError, Token, tokenize};

/// A structural type expression, appearing after the `'` quote marker.
#[derive(Debug, PartialEq, Clone)]
pub enum TypeExpr {
    /// A named primitive or user-defined type: `i64`, `f64`, `bool`, `myType`.
    Named(String),
    /// A wildcard type `_` — matches any type.
    Wildcard,
    /// An array / list type: `[T]`.
    Array(Box<TypeExpr>),
    /// A map type: `{k1: T1, k2: T2, ...}`.
    Map(Vec<(String, TypeExpr)>),
    /// A function type written with pipe notation: `(InputType | OutputType)`.
    Function(Box<TypeExpr>, Box<TypeExpr>),
    /// A generic type with explicit type parameters: `<T constraint, ...> body`.
    ///
    /// Each entry in the `Vec` is `(param_name, constraint)`.  The constraint
    /// is itself a `TypeExpr`; a `Wildcard` constraint means unconstrained.
    Generic(Vec<(String, TypeExpr)>, Box<TypeExpr>),
}

/// An expression in the AST.
#[derive(Debug, PartialEq, Clone)]
pub enum Expr {
    Number(f64),
    Bool(bool),
    Str(String),
    Symbol(String),
    /// A structural type expression created by the `'` marker: `'i64`, `'[bool]`, etc.
    StructuralType(TypeExpr),
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
    /// A `.` appeared where it cannot start an expression.
    UnexpectedDot,
    /// A `.` was not followed by a symbol name.
    InvalidDotAccess,
    /// A structural type expression after `'` was malformed.
    InvalidTypeExpr,
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
            ParseErrorKind::UnexpectedDot => {
                write!(f, "unexpected '.' — dot access must follow an expression")
            }
            ParseErrorKind::InvalidDotAccess => {
                write!(f, "expected a field or method name after '.'")
            }
            ParseErrorKind::InvalidTypeExpr => {
                write!(
                    f,
                    "invalid type expression after ''' — expected a type name, '[T]', '{{k: T}}', '(A | B)', or '<T C> body'"
                )
            }
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

/// True when the tokens at `pos` unambiguously begin a new function definition.
///
/// Three forms are recognised:
///
/// 1. `name: body`              — plain definition
/// 2. `name typeRef: body`      — named type annotation
/// 3. `name 'typeExpr: body`    — inline structural type annotation
fn is_func_def_start(tokens: &[(Token, usize)], pos: usize) -> bool {
    if !matches!(&tokens[pos].0, Token::Symbol(_)) {
        return false;
    }
    // Case 1: `name: body`  (no annotation)
    if pos + 1 < tokens.len() && tokens[pos + 1].0 == Token::Colon {
        return true;
    }
    // Case 2: `name typeRef: body`  (named type annotation)
    if pos + 2 < tokens.len()
        && matches!(&tokens[pos + 1].0, Token::Symbol(s) if s != "_")
        && tokens[pos + 2].0 == Token::Colon
    {
        return true;
    }
    // Case 3: `name 'typeExpr: body`  (inline structural type annotation)
    if pos + 1 < tokens.len() && tokens[pos + 1].0 == Token::Quote {
        return true;
    }
    false
}

/// True only for the original minimal `Symbol :` form.
///
/// Used by the *missing-body* guard inside `parse()` to avoid a false
/// positive when a body expression is a plain symbol that happens to be
/// followed by the start of the next definition (which would match the
/// case-2 pattern `Symbol Symbol :`).
fn is_plain_func_def_start(tokens: &[(Token, usize)], pos: usize) -> bool {
    matches!(&tokens[pos].0, Token::Symbol(_))
        && pos + 1 < tokens.len()
        && tokens[pos + 1].0 == Token::Colon
}

/// Parse a source string into a list of top-level function definitions.
///
/// Only function definitions are allowed at the top level.  Three forms:
///
/// - `name: body`              — function with implicit parameter `x`
/// - `name typeRef: body`      — function annotated with a named type
/// - `name 'typeExpr: body`    — function annotated with an inline structural type
///
/// If the body does not reference `x`, the function behaves as a
/// zero-argument function. The body is exactly one expression (monadic).
/// Multi-arg functions do not exist; pass a tuple `[a, b]` to built-in
/// operators instead.
///
/// Annotated definitions desugar to `(define (name x) body annotation)` where
/// `annotation` is either an `Expr::Symbol` (named) or `Expr::StructuralType`
/// (inline).  Unannotated definitions omit the fourth element.
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
        pos += 1; // consume name

        // Consume the optional type annotation, then the colon.
        let annotation: Option<Expr> = if pos < tokens.len() && tokens[pos].0 == Token::Colon {
            // Plain `name: body` — no annotation.
            pos += 1; // consume `:`
            None
        } else if pos + 1 < tokens.len()
            && matches!(&tokens[pos].0, Token::Symbol(_))
            && tokens[pos + 1].0 == Token::Colon
        {
            // `name typeRef: body` — named type annotation.
            let type_name = match tokens[pos].0.clone() {
                Token::Symbol(s) => s,
                _ => unreachable!(),
            };
            pos += 1; // consume type name
            pos += 1; // consume `:`
            Some(Expr::Symbol(type_name))
        } else if pos < tokens.len() && tokens[pos].0 == Token::Quote {
            // `name 'typeExpr: body` — inline structural type annotation.
            pos += 1; // consume `'`
            let ty = parse_type_expr(&tokens, &mut pos)?;
            if pos >= tokens.len() || tokens[pos].0 != Token::Colon {
                return Err(ParseError {
                    kind: ParseErrorKind::InvalidFuncDef,
                    line: def_line,
                });
            }
            pos += 1; // consume `:`
            Some(Expr::StructuralType(ty))
        } else {
            return Err(ParseError {
                kind: ParseErrorKind::InvalidFuncDef,
                line: def_line,
            });
        };

        // All functions have an implicit parameter `x`.
        let param_names: Vec<String> = vec!["x".to_string()];

        // Parse exactly one body expression.
        //
        // Use `is_plain_func_def_start` (Symbol Colon only) here — the full
        // `is_func_def_start` would trigger a false positive when the body is
        // a plain symbol followed by `nextName: …` (matching case 2).
        if pos >= tokens.len() || is_plain_func_def_start(&tokens, pos) {
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

        // Desugar to `(define (name x) body)` or
        //             `(define (name x) body annotation)` for typed definitions.
        let mut sig = vec![Expr::Symbol(name)];
        sig.extend(param_names.into_iter().map(Expr::Symbol));
        let mut items = vec![Expr::Symbol("define".to_string()), Expr::List(sig), body];
        if let Some(ann) = annotation {
            items.push(ann);
        }
        exprs.push(Expr::List(items));
    }

    Ok(exprs)
}

// ---------------------------------------------------------------------------
// Structural type expression parser
// ---------------------------------------------------------------------------

/// Parse a structural type expression starting at `*pos`.
///
/// Grammar (after the leading `'` has been consumed):
///
/// ```text
/// type-expr  ::= named-type | wildcard | array-type | map-type | func-type
/// named-type ::= Symbol              (e.g. `i64`, `f64`, `bool`, `myType`)
/// wildcard   ::= `_`
/// array-type ::= `[` type-expr `]`
/// map-type   ::= `{` (Symbol `:` type-expr (`,`)?)* `}`
/// func-type  ::= `(` type-expr `|` type-expr `)`
/// ```
fn parse_type_expr(tokens: &[(Token, usize)], pos: &mut usize) -> Result<TypeExpr, ParseError> {
    if *pos >= tokens.len() {
        return Err(ParseError {
            kind: ParseErrorKind::MissingQuoteTarget,
            line: line_at(tokens, *pos),
        });
    }
    let tok_line = tokens[*pos].1;

    match tokens[*pos].0.clone() {
        // Wildcard `_`
        Token::Symbol(s) if s == "_" => {
            *pos += 1;
            Ok(TypeExpr::Wildcard)
        }

        // Named type: any symbol that is not `_`
        Token::Symbol(s) => {
            *pos += 1;
            Ok(TypeExpr::Named(s))
        }

        // Array type: `[T]`
        Token::LBracket => {
            *pos += 1; // consume `[`
            let inner = parse_type_expr(tokens, pos)?;
            if *pos >= tokens.len() || tokens[*pos].0 != Token::RBracket {
                return Err(ParseError {
                    kind: ParseErrorKind::UnmatchedOpenBracket,
                    line: tok_line,
                });
            }
            *pos += 1; // consume `]`
            Ok(TypeExpr::Array(Box::new(inner)))
        }

        // Map type: `{k1: T1, k2: T2, ...}`
        Token::LBrace => {
            *pos += 1; // consume `{`
            let mut entries: Vec<(String, TypeExpr)> = Vec::new();
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
                // Expect: Symbol `:` type-expr
                let key = match tokens[*pos].0.clone() {
                    Token::Symbol(s) => s,
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
                *pos += 1; // consume `:`
                let val_ty = parse_type_expr(tokens, pos)?;
                entries.push((key, val_ty));
                // Skip optional comma separator.
                if *pos < tokens.len() && tokens[*pos].0 == Token::Comma {
                    *pos += 1;
                }
            }
            Ok(TypeExpr::Map(entries))
        }

        // Function type: `(InputType | OutputType)`  or grouped type: `(T)`
        Token::LParen => {
            *pos += 1; // consume `(`
            let input_ty = parse_type_expr(tokens, pos)?;
            if *pos < tokens.len() && tokens[*pos].0 == Token::Pipe {
                // Function type `(A | B)`
                *pos += 1; // consume `|`
                let output_ty = parse_type_expr(tokens, pos)?;
                if *pos >= tokens.len() || tokens[*pos].0 != Token::RParen {
                    return Err(ParseError {
                        kind: ParseErrorKind::UnmatchedOpenParen,
                        line: tok_line,
                    });
                }
                *pos += 1; // consume `)`
                Ok(TypeExpr::Function(Box::new(input_ty), Box::new(output_ty)))
            } else {
                // Grouped type `(T)` — just unwrap the inner type.
                if *pos >= tokens.len() || tokens[*pos].0 != Token::RParen {
                    return Err(ParseError {
                        kind: ParseErrorKind::UnmatchedOpenParen,
                        line: tok_line,
                    });
                }
                *pos += 1; // consume `)`
                Ok(input_ty)
            }
        }

        // Generic type: `<T constraint, U constraint, ...> body`
        Token::LAngle => {
            *pos += 1; // consume `<`
            let mut params: Vec<(String, TypeExpr)> = Vec::new();
            loop {
                if *pos >= tokens.len() {
                    return Err(ParseError {
                        kind: ParseErrorKind::InvalidTypeExpr,
                        line: tok_line,
                    });
                }
                if tokens[*pos].0 == Token::RAngle {
                    *pos += 1; // consume `>`
                    break;
                }
                // Expect: Symbol type-expr
                let param_name = match tokens[*pos].0.clone() {
                    Token::Symbol(s) => s,
                    _ => {
                        return Err(ParseError {
                            kind: ParseErrorKind::InvalidTypeExpr,
                            line: tokens[*pos].1,
                        });
                    }
                };
                *pos += 1; // consume param name
                let constraint = parse_type_expr(tokens, pos)?;
                params.push((param_name, constraint));
                // Skip optional comma separator.
                if *pos < tokens.len() && tokens[*pos].0 == Token::Comma {
                    *pos += 1;
                }
            }
            // Parse the body type that follows the `<...>`.
            let body = parse_type_expr(tokens, pos)?;
            Ok(TypeExpr::Generic(params, Box::new(body)))
        }

        _ => Err(ParseError {
            kind: ParseErrorKind::InvalidTypeExpr,
            line: tok_line,
        }),
    }
}

/// Returns `true` if `token` can legally start a primary expression.
fn can_start_expr(token: &Token) -> bool {
    matches!(
        token,
        Token::LParen
            | Token::LBracket
            | Token::LBrace
            | Token::Quote
            | Token::Number(_)
            | Token::Bool(_)
            | Token::Str(_)
            | Token::Symbol(_)
    )
}

/// Parse a single expression from `tokens` starting at `*pos`, including any
/// trailing dot-access / dot-method postfix chains.
///
/// `expr.key`        desugars to `(get [expr, key])` (map field access).
/// `expr.method arg` desugars to `(method [expr, arg])` (binary method call).
fn parse_expr(tokens: &[(Token, usize)], pos: &mut usize) -> Result<Expr, ParseError> {
    let mut expr = parse_primary(tokens, pos)?;

    // Left-to-right dot-chain loop.
    while *pos < tokens.len() && tokens[*pos].0 == Token::Dot {
        let dot_line = tokens[*pos].1;
        *pos += 1; // consume `.`

        // Must be followed by a symbol (the field/method name).
        let name = match *pos < tokens.len() {
            true => match tokens[*pos].0.clone() {
                Token::Symbol(s) => s,
                _ => {
                    return Err(ParseError {
                        kind: ParseErrorKind::InvalidDotAccess,
                        line: tokens[*pos].1,
                    });
                }
            },
            false => {
                return Err(ParseError {
                    kind: ParseErrorKind::InvalidDotAccess,
                    line: dot_line,
                });
            }
        };
        *pos += 1; // consume symbol

        // If the next token can start an expression, treat this as a binary
        // method call: `lhs.method rhs` → `(method [lhs, rhs])`.
        // Otherwise it's a map field access: `lhs.key` → `(get [lhs, key])`.
        if *pos < tokens.len() && can_start_expr(&tokens[*pos].0) {
            let rhs = parse_primary(tokens, pos)?;
            expr = Expr::List(vec![Expr::Symbol(name), Expr::Tuple(vec![expr, rhs])]);
        } else {
            expr = Expr::List(vec![
                Expr::Symbol("get".to_string()),
                Expr::Tuple(vec![expr, Expr::Symbol(name)]),
            ]);
        }
    }

    Ok(expr)
}

/// Parse a single primary expression from `tokens` starting at `*pos`.
/// Does not consume any trailing dot-access chains; call `parse_expr` for that.
fn parse_primary(tokens: &[(Token, usize)], pos: &mut usize) -> Result<Expr, ParseError> {
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
        Token::Dot => Err(ParseError {
            kind: ParseErrorKind::UnexpectedDot,
            line: tok_line,
        }),

        Token::LAngle | Token::RAngle => Err(ParseError {
            kind: ParseErrorKind::InvalidTypeExpr,
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
            let ty = parse_type_expr(tokens, pos)?;
            Ok(Expr::StructuralType(ty))
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

    // ---- structural types (`'`) -----------------------------------------------

    #[test]
    fn test_structural_type_named_i64() {
        // `anyInt: 'i64` desugars to a define whose body is StructuralType(Named("i64"))
        let sym = |s: &str| Expr::Symbol(s.to_string());
        let result = parse("anyInt: 'i64").unwrap();
        assert_eq!(
            result[0],
            Expr::List(vec![
                sym("define"),
                Expr::List(vec![sym("anyInt"), sym("x")]),
                Expr::StructuralType(TypeExpr::Named("i64".to_string())),
            ])
        );
    }

    #[test]
    fn test_structural_type_wildcard() {
        // `f: '_` — wildcard structural type
        let sym = |s: &str| Expr::Symbol(s.to_string());
        let result = parse("f: '_").unwrap();
        assert_eq!(
            result[0],
            Expr::List(vec![
                sym("define"),
                Expr::List(vec![sym("f"), sym("x")]),
                Expr::StructuralType(TypeExpr::Wildcard),
            ])
        );
    }

    #[test]
    fn test_structural_type_array() {
        // `anyIntArray: '[i64]`
        let sym = |s: &str| Expr::Symbol(s.to_string());
        let result = parse("anyIntArray: '[i64]").unwrap();
        assert_eq!(
            result[0],
            Expr::List(vec![
                sym("define"),
                Expr::List(vec![sym("anyIntArray"), sym("x")]),
                Expr::StructuralType(TypeExpr::Array(Box::new(TypeExpr::Named(
                    "i64".to_string()
                )))),
            ])
        );
    }

    #[test]
    fn test_structural_type_nested_array() {
        // `twoDim: '[[i64]]`
        let sym = |s: &str| Expr::Symbol(s.to_string());
        let result = parse("twoDim: '[[i64]]").unwrap();
        assert_eq!(
            result[0],
            Expr::List(vec![
                sym("define"),
                Expr::List(vec![sym("twoDim"), sym("x")]),
                Expr::StructuralType(TypeExpr::Array(Box::new(TypeExpr::Array(Box::new(
                    TypeExpr::Named("i64".to_string())
                ))))),
            ])
        );
    }

    #[test]
    fn test_structural_type_map() {
        // `anyMap: '{k1: bool, k2: i64}`
        let sym = |s: &str| Expr::Symbol(s.to_string());
        let result = parse("anyMap: '{k1: bool, k2: i64}").unwrap();
        assert_eq!(
            result[0],
            Expr::List(vec![
                sym("define"),
                Expr::List(vec![sym("anyMap"), sym("x")]),
                Expr::StructuralType(TypeExpr::Map(vec![
                    ("k1".to_string(), TypeExpr::Named("bool".to_string())),
                    ("k2".to_string(), TypeExpr::Named("i64".to_string())),
                ])),
            ])
        );
    }

    #[test]
    fn test_structural_type_function() {
        // `anyFn: '(i64 | bool)` — function type
        let sym = |s: &str| Expr::Symbol(s.to_string());
        let result = parse("anyFn: '(i64 | bool)").unwrap();
        assert_eq!(
            result[0],
            Expr::List(vec![
                sym("define"),
                Expr::List(vec![sym("anyFn"), sym("x")]),
                Expr::StructuralType(TypeExpr::Function(
                    Box::new(TypeExpr::Named("i64".to_string())),
                    Box::new(TypeExpr::Named("bool".to_string())),
                )),
            ])
        );
    }

    #[test]
    fn test_structural_type_function_wildcard() {
        // `discard: '(_|_)` — both input and output wildcarded
        let sym = |s: &str| Expr::Symbol(s.to_string());
        let result = parse("discard: '(_|_)").unwrap();
        assert_eq!(
            result[0],
            Expr::List(vec![
                sym("define"),
                Expr::List(vec![sym("discard"), sym("x")]),
                Expr::StructuralType(TypeExpr::Function(
                    Box::new(TypeExpr::Wildcard),
                    Box::new(TypeExpr::Wildcard),
                )),
            ])
        );
    }

    #[test]
    fn test_named_type_annotation() {
        // `labelUsage anyFloat: 2` — named type annotation on function def
        let sym = |s: &str| Expr::Symbol(s.to_string());
        let result = parse("labelUsage anyFloat: 2").unwrap();
        assert_eq!(
            result[0],
            Expr::List(vec![
                sym("define"),
                Expr::List(vec![sym("labelUsage"), sym("x")]),
                Expr::Number(2.0),
                sym("anyFloat"),
            ])
        );
    }

    #[test]
    fn test_inline_type_annotation() {
        // `typed 'i64: 42` — inline structural type annotation on function def
        let sym = |s: &str| Expr::Symbol(s.to_string());
        let result = parse("typed 'i64: 42").unwrap();
        assert_eq!(
            result[0],
            Expr::List(vec![
                sym("define"),
                Expr::List(vec![sym("typed"), sym("x")]),
                Expr::Number(42.0),
                Expr::StructuralType(TypeExpr::Named("i64".to_string())),
            ])
        );
    }

    #[test]
    fn test_inline_function_type_annotation() {
        // `not '(bool | bool): 0` — inline function type annotation
        let sym = |s: &str| Expr::Symbol(s.to_string());
        let result = parse("not '(bool | bool): 0").unwrap();
        assert_eq!(
            result[0],
            Expr::List(vec![
                sym("define"),
                Expr::List(vec![sym("not"), sym("x")]),
                Expr::Number(0.0),
                Expr::StructuralType(TypeExpr::Function(
                    Box::new(TypeExpr::Named("bool".to_string())),
                    Box::new(TypeExpr::Named("bool".to_string())),
                )),
            ])
        );
    }

    #[test]
    fn test_structural_type_in_expr_position() {
        // `f: 'bool` as a body expr — StructuralType in value position
        let sym = |s: &str| Expr::Symbol(s.to_string());
        let result = parse("f: 'bool").unwrap();
        assert_eq!(
            result[0],
            Expr::List(vec![
                sym("define"),
                Expr::List(vec![sym("f"), sym("x")]),
                Expr::StructuralType(TypeExpr::Named("bool".to_string())),
            ])
        );
    }

    #[test]
    fn test_invalid_type_expr_error() {
        // `'42` is not a valid type expression (number, not a type name/shape)
        assert_eq!(parse_err_kind("f: '42"), ParseErrorKind::InvalidTypeExpr);
    }

    // ---- generic type expressions (`'<T C> body`) ----------------------------

    #[test]
    fn test_generic_wildcard_function_type() {
        // `genericId: '<T _> (T | T)` — identity for any type T
        let sym = |s: &str| Expr::Symbol(s.to_string());
        let result = parse("genericId: '<T _> (T | T)").unwrap();
        assert_eq!(
            result[0],
            Expr::List(vec![
                sym("define"),
                Expr::List(vec![sym("genericId"), sym("x")]),
                Expr::StructuralType(TypeExpr::Generic(
                    vec![("T".to_string(), TypeExpr::Wildcard)],
                    Box::new(TypeExpr::Function(
                        Box::new(TypeExpr::Named("T".to_string())),
                        Box::new(TypeExpr::Named("T".to_string())),
                    )),
                )),
            ])
        );
    }

    #[test]
    fn test_generic_constrained_map_type() {
        // `pairOfSame: '<T _> {k1: T, k2: T}`
        let sym = |s: &str| Expr::Symbol(s.to_string());
        let result = parse("pairOfSame: '<T _> {k1: T, k2: T}").unwrap();
        assert_eq!(
            result[0],
            Expr::List(vec![
                sym("define"),
                Expr::List(vec![sym("pairOfSame"), sym("x")]),
                Expr::StructuralType(TypeExpr::Generic(
                    vec![("T".to_string(), TypeExpr::Wildcard)],
                    Box::new(TypeExpr::Map(vec![
                        ("k1".to_string(), TypeExpr::Named("T".to_string())),
                        ("k2".to_string(), TypeExpr::Named("T".to_string())),
                    ])),
                )),
            ])
        );
    }

    #[test]
    fn test_generic_multiple_params() {
        // `multi: '<T i64, U _> {k1: T, k2: U}`
        let sym = |s: &str| Expr::Symbol(s.to_string());
        let result = parse("multi: '<T i64, U _> {k1: T, k2: U}").unwrap();
        assert_eq!(
            result[0],
            Expr::List(vec![
                sym("define"),
                Expr::List(vec![sym("multi"), sym("x")]),
                Expr::StructuralType(TypeExpr::Generic(
                    vec![
                        ("T".to_string(), TypeExpr::Named("i64".to_string())),
                        ("U".to_string(), TypeExpr::Wildcard),
                    ],
                    Box::new(TypeExpr::Map(vec![
                        ("k1".to_string(), TypeExpr::Named("T".to_string())),
                        ("k2".to_string(), TypeExpr::Named("U".to_string())),
                    ])),
                )),
            ])
        );
    }

    #[test]
    fn test_generic_nested_constraint() {
        // `nested: '<T i64, U <V _>{k1: V}> {k1: T}` — nested generic in constraint
        let sym = |s: &str| Expr::Symbol(s.to_string());
        let result = parse("nested: '<T i64, U <V _>{k1: V}> {k1: T}").unwrap();
        assert_eq!(
            result[0],
            Expr::List(vec![
                sym("define"),
                Expr::List(vec![sym("nested"), sym("x")]),
                Expr::StructuralType(TypeExpr::Generic(
                    vec![
                        ("T".to_string(), TypeExpr::Named("i64".to_string())),
                        (
                            "U".to_string(),
                            TypeExpr::Generic(
                                vec![("V".to_string(), TypeExpr::Wildcard)],
                                Box::new(TypeExpr::Map(vec![(
                                    "k1".to_string(),
                                    TypeExpr::Named("V".to_string()),
                                )])),
                            ),
                        ),
                    ],
                    Box::new(TypeExpr::Map(vec![(
                        "k1".to_string(),
                        TypeExpr::Named("T".to_string()),
                    )])),
                )),
            ])
        );
    }

    #[test]
    fn test_generic_unclosed_angle_bracket_error() {
        // `'<T _` — missing closing `>`
        assert_eq!(parse_err_kind("f: '<T _"), ParseErrorKind::InvalidTypeExpr);
    }
}
