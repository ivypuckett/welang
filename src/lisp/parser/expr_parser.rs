use crate::lisp::lexer::Token;
use crate::lisp::parser::type_parser::parse_type_expr;
use crate::lisp::parser::types::{Expr, ParseError, ParseErrorKind};

pub fn can_start_expr(token: &Token) -> bool {
    matches!(
        token,
        Token::LParen
            | Token::LBracket
            | Token::LBrace
            | Token::Quote
            | Token::Star
            | Token::Number(_)
            | Token::Bool(_)
            | Token::Str(_)
            | Token::Symbol(_)
    )
}

pub fn parse_expr(tokens: &[(Token, usize)], pos: &mut usize) -> Result<Expr, ParseError> {
    let mut expr = parse_primary(tokens, pos)?;
    while *pos < tokens.len() && tokens[*pos].0 == Token::Dot {
        let dot_line = tokens[*pos].1;
        *pos += 1;
        let name = if *pos < tokens.len() {
            match tokens[*pos].0.clone() {
                Token::Symbol(s) => s,
                _ => {
                    return Err(ParseError {
                        kind: ParseErrorKind::InvalidDotAccess,
                        line: tokens[*pos].1,
                    });
                }
            }
        } else {
            return Err(ParseError {
                kind: ParseErrorKind::InvalidDotAccess,
                line: dot_line,
            });
        };
        *pos += 1;
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

pub fn parse_primary(tokens: &[(Token, usize)], pos: &mut usize) -> Result<Expr, ParseError> {
    if *pos >= tokens.len() {
        return Err(ParseError {
            kind: ParseErrorKind::UnmatchedOpenParen,
            line: super::line_at(tokens, *pos),
        });
    }
    let tok_line = tokens[*pos].1;
    match tokens[*pos].0.clone() {
        Token::LParen => {
            *pos += 1;
            if *pos < tokens.len()
                && matches!(&tokens[*pos].0, Token::Symbol(_))
                && *pos + 1 < tokens.len()
                && tokens[*pos + 1].0 == Token::Colon
            {
                let rename = match &tokens[*pos].0 {
                    Token::Symbol(s) => s.clone(),
                    _ => unreachable!(),
                };
                *pos += 2;
                let body = parse_expr(tokens, pos)?;
                if *pos >= tokens.len() || tokens[*pos].0 != Token::RParen {
                    return Err(ParseError {
                        kind: ParseErrorKind::UnmatchedOpenParen,
                        line: tok_line,
                    });
                }
                *pos += 1;
                return Ok(Expr::Rename(rename, Box::new(body)));
            }
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
                Ok(Expr::List(segments.into_iter().next().unwrap()))
            } else {
                let mut iter = segments.into_iter();
                let first = iter.next().unwrap();
                let mut acc = fold_pipe(first, None).map_err(|kind| ParseError {
                    kind,
                    line: tok_line,
                })?;
                for seg in iter {
                    acc = fold_pipe(seg, Some(acc)).map_err(|kind| ParseError {
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
                if *pos < tokens.len() && tokens[*pos].0 == Token::Comma {
                    *pos += 1;
                }
            }
            Ok(Expr::Tuple(items))
        }
        Token::LBrace => {
            *pos += 1;
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
                            line: super::line_at(tokens, *pos),
                        });
                    }
                    *pos += 1;
                    let val = parse_expr(tokens, pos)?;
                    entries.push((key, val));
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
                    let key = match &tokens[*pos].0 {
                        Token::Symbol(s) => s.clone(),
                        _ => {
                            return Err(ParseError {
                                kind: ParseErrorKind::InvalidMapEntry,
                                line: tokens[*pos].1,
                            });
                        }
                    };
                    *pos += 1;
                    if *pos >= tokens.len() || tokens[*pos].0 != Token::Colon {
                        return Err(ParseError {
                            kind: ParseErrorKind::InvalidMapEntry,
                            line: super::line_at(tokens, *pos),
                        });
                    }
                    *pos += 1;
                    let val = parse_expr(tokens, pos)?;
                    entries.push((key, val));
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
            kind: ParseErrorKind::UnexpectedCloseBracket,
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
        Token::Star => {
            *pos += 1;
            if *pos >= tokens.len() {
                return Err(ParseError {
                    kind: ParseErrorKind::MissingQuoteTarget,
                    line: tok_line,
                });
            }
            let ty = parse_type_expr(tokens, pos)?;
            Ok(Expr::NominalType(ty))
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

fn fold_pipe(items: Vec<Expr>, seed: Option<Expr>) -> Result<Expr, ParseErrorKind> {
    if items.is_empty() {
        return Err(ParseErrorKind::EmptyPipeSegment);
    }
    let mut iter = items.into_iter().rev();
    let last = iter.next().unwrap();
    let mut acc = match seed {
        None => last,
        Some(arg) => Expr::List(vec![last, arg]),
    };
    for func in iter {
        acc = Expr::List(vec![func, acc]);
    }
    Ok(acc)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lisp::parser::parse;

    fn sym(s: &str) -> Expr {
        Expr::Symbol(s.to_string())
    }
    fn def(name: &str, body: Expr) -> Expr {
        Expr::List(vec![
            sym("define"),
            Expr::List(vec![sym(name), sym("x")]),
            body,
        ])
    }
    fn call(f: Expr, a: Expr) -> Expr {
        Expr::List(vec![f, a])
    }

    #[test]
    fn test_simple_pipe() {
        assert_eq!(
            parse("f: (x | double)").unwrap(),
            vec![def("f", call(sym("double"), sym("x")))]
        );
    }

    #[test]
    fn test_chained_pipe() {
        assert_eq!(
            parse("f: (x | double | double)").unwrap(),
            vec![def("f", call(sym("double"), call(sym("double"), sym("x"))))]
        );
    }

    #[test]
    fn test_pipe_with_call_on_left() {
        assert_eq!(
            parse("f: (double x | inc)").unwrap(),
            vec![def("f", call(sym("inc"), call(sym("double"), sym("x"))))]
        );
    }

    #[test]
    fn test_pipe_multi_element_segments() {
        let expected_body = call(
            sym("n5"),
            call(
                sym("n4"),
                call(sym("n3"), call(sym("n2"), call(sym("n1"), sym("x")))),
            ),
        );
        assert_eq!(
            parse("f: (n3 n2 n1 x | n5 n4)").unwrap(),
            vec![def("f", expected_body)]
        );
    }

    #[test]
    fn test_pipe_empty_segment_error() {
        assert_eq!(
            parse("f: (x | | double)").unwrap_err().kind,
            ParseErrorKind::EmptyPipeSegment
        );
    }

    #[test]
    fn test_pipe_leading_pipe_error() {
        assert_eq!(
            parse("f: (| double)").unwrap_err().kind,
            ParseErrorKind::EmptyPipeSegment
        );
    }

    #[test]
    fn test_pipe_trailing_pipe_error() {
        assert_eq!(
            parse("f: (double |)").unwrap_err().kind,
            ParseErrorKind::EmptyPipeSegment
        );
    }
}
