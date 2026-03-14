use crate::lisp::lexer::Token;
use crate::lisp::parser::types::{ParseError, ParseErrorKind, TypeExpr};

pub fn parse_type_expr(tokens: &[(Token, usize)], pos: &mut usize) -> Result<TypeExpr, ParseError> {
    if *pos >= tokens.len() {
        return Err(ParseError {
            kind: ParseErrorKind::MissingQuoteTarget,
            line: super::line_at(tokens, *pos),
        });
    }
    let tok_line = tokens[*pos].1;
    match tokens[*pos].0.clone() {
        Token::Symbol(s) if s == "_" => {
            *pos += 1;
            Ok(TypeExpr::Wildcard)
        }
        Token::Symbol(s) => {
            *pos += 1;
            Ok(TypeExpr::Named(s))
        }
        Token::LBracket => {
            *pos += 1;
            let inner = parse_type_expr(tokens, pos)?;
            if *pos >= tokens.len() || tokens[*pos].0 != Token::RBracket {
                return Err(ParseError {
                    kind: ParseErrorKind::UnmatchedOpenBracket,
                    line: tok_line,
                });
            }
            *pos += 1;
            Ok(TypeExpr::Array(Box::new(inner)))
        }
        Token::LBrace => {
            *pos += 1;
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
                let key = match tokens[*pos].0.clone() {
                    Token::Symbol(s) => s,
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
                let val_ty = parse_type_expr(tokens, pos)?;
                entries.push((key, val_ty));
                if *pos < tokens.len() && tokens[*pos].0 == Token::Comma {
                    *pos += 1;
                }
            }
            Ok(TypeExpr::Map(entries))
        }
        Token::LParen => {
            *pos += 1;
            let input_ty = parse_type_expr(tokens, pos)?;
            if *pos < tokens.len() && tokens[*pos].0 == Token::Pipe {
                *pos += 1;
                let output_ty = parse_type_expr(tokens, pos)?;
                if *pos >= tokens.len() || tokens[*pos].0 != Token::RParen {
                    return Err(ParseError {
                        kind: ParseErrorKind::UnmatchedOpenParen,
                        line: tok_line,
                    });
                }
                *pos += 1;
                Ok(TypeExpr::Function(Box::new(input_ty), Box::new(output_ty)))
            } else {
                if *pos >= tokens.len() || tokens[*pos].0 != Token::RParen {
                    return Err(ParseError {
                        kind: ParseErrorKind::UnmatchedOpenParen,
                        line: tok_line,
                    });
                }
                *pos += 1;
                Ok(input_ty)
            }
        }
        Token::LAngle => {
            *pos += 1;
            let mut params: Vec<(String, TypeExpr)> = Vec::new();
            loop {
                if *pos >= tokens.len() {
                    return Err(ParseError {
                        kind: ParseErrorKind::InvalidTypeExpr,
                        line: tok_line,
                    });
                }
                if tokens[*pos].0 == Token::RAngle {
                    *pos += 1;
                    break;
                }
                let param_name = match tokens[*pos].0.clone() {
                    Token::Symbol(s) => s,
                    _ => {
                        return Err(ParseError {
                            kind: ParseErrorKind::InvalidTypeExpr,
                            line: tokens[*pos].1,
                        });
                    }
                };
                *pos += 1;
                let constraint = parse_type_expr(tokens, pos)?;
                params.push((param_name, constraint));
                if *pos < tokens.len() && tokens[*pos].0 == Token::Comma {
                    *pos += 1;
                }
            }
            let body = parse_type_expr(tokens, pos)?;
            Ok(TypeExpr::Generic(params, Box::new(body)))
        }
        Token::Star => {
            *pos += 1;
            let inner = parse_type_expr(tokens, pos)?;
            Ok(TypeExpr::Nominal(Box::new(inner)))
        }
        _ => Err(ParseError {
            kind: ParseErrorKind::InvalidTypeExpr,
            line: tok_line,
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lisp::parser::parse;
    use crate::lisp::parser::types::Expr;

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
    fn def_ann(name: &str, body: Expr, ann: Expr) -> Expr {
        Expr::List(vec![
            sym("define"),
            Expr::List(vec![sym(name), sym("x")]),
            body,
            ann,
        ])
    }

    #[test]
    fn test_structural_type_named_i64() {
        assert_eq!(
            parse("anyInt: 'i64").unwrap(),
            vec![def(
                "anyInt",
                Expr::StructuralType(TypeExpr::Named("i64".to_string()))
            )]
        );
    }

    #[test]
    fn test_structural_type_wildcard() {
        assert_eq!(
            parse("f: '_").unwrap(),
            vec![def("f", Expr::StructuralType(TypeExpr::Wildcard))]
        );
    }

    #[test]
    fn test_structural_type_array() {
        assert_eq!(
            parse("anyIntArray: '[i64]").unwrap(),
            vec![def(
                "anyIntArray",
                Expr::StructuralType(TypeExpr::Array(Box::new(TypeExpr::Named(
                    "i64".to_string()
                ))))
            )]
        );
    }

    #[test]
    fn test_structural_type_nested_array() {
        assert_eq!(
            parse("twoDim: '[[i64]]").unwrap(),
            vec![def(
                "twoDim",
                Expr::StructuralType(TypeExpr::Array(Box::new(TypeExpr::Array(Box::new(
                    TypeExpr::Named("i64".to_string())
                )))))
            )]
        );
    }

    #[test]
    fn test_structural_type_map() {
        assert_eq!(
            parse("anyMap: '{k1: bool, k2: i64}").unwrap(),
            vec![def(
                "anyMap",
                Expr::StructuralType(TypeExpr::Map(vec![
                    ("k1".to_string(), TypeExpr::Named("bool".to_string())),
                    ("k2".to_string(), TypeExpr::Named("i64".to_string())),
                ]))
            )]
        );
    }

    #[test]
    fn test_structural_type_function() {
        assert_eq!(
            parse("anyFn: '(i64 | bool)").unwrap(),
            vec![def(
                "anyFn",
                Expr::StructuralType(TypeExpr::Function(
                    Box::new(TypeExpr::Named("i64".to_string())),
                    Box::new(TypeExpr::Named("bool".to_string())),
                ))
            )]
        );
    }

    #[test]
    fn test_structural_type_function_wildcard() {
        assert_eq!(
            parse("discard: '(_|_)").unwrap(),
            vec![def(
                "discard",
                Expr::StructuralType(TypeExpr::Function(
                    Box::new(TypeExpr::Wildcard),
                    Box::new(TypeExpr::Wildcard),
                ))
            )]
        );
    }

    #[test]
    fn test_named_type_annotation() {
        assert_eq!(
            parse("labelUsage anyFloat: 2").unwrap(),
            vec![def_ann("labelUsage", Expr::Number(2.0), sym("anyFloat"))]
        );
    }

    #[test]
    fn test_inline_type_annotation() {
        assert_eq!(
            parse("typed 'i64: 42").unwrap(),
            vec![def_ann(
                "typed",
                Expr::Number(42.0),
                Expr::StructuralType(TypeExpr::Named("i64".to_string()))
            )]
        );
    }

    #[test]
    fn test_inline_function_type_annotation() {
        assert_eq!(
            parse("not '(bool | bool): 0").unwrap(),
            vec![def_ann(
                "not",
                Expr::Number(0.0),
                Expr::StructuralType(TypeExpr::Function(
                    Box::new(TypeExpr::Named("bool".to_string())),
                    Box::new(TypeExpr::Named("bool".to_string())),
                ))
            )]
        );
    }

    #[test]
    fn test_structural_type_in_expr_position() {
        assert_eq!(
            parse("f: 'bool").unwrap(),
            vec![def(
                "f",
                Expr::StructuralType(TypeExpr::Named("bool".to_string()))
            )]
        );
    }

    #[test]
    fn test_invalid_type_expr_error() {
        assert_eq!(
            parse("f: '42").unwrap_err().kind,
            ParseErrorKind::InvalidTypeExpr
        );
    }

    #[test]
    fn test_generic_wildcard_function_type() {
        assert_eq!(
            parse("genericId: '<T _> (T | T)").unwrap(),
            vec![def(
                "genericId",
                Expr::StructuralType(TypeExpr::Generic(
                    vec![("T".to_string(), TypeExpr::Wildcard)],
                    Box::new(TypeExpr::Function(
                        Box::new(TypeExpr::Named("T".to_string())),
                        Box::new(TypeExpr::Named("T".to_string())),
                    )),
                ))
            )]
        );
    }

    #[test]
    fn test_generic_constrained_map_type() {
        assert_eq!(
            parse("pairOfSame: '<T _> {k1: T, k2: T}").unwrap(),
            vec![def(
                "pairOfSame",
                Expr::StructuralType(TypeExpr::Generic(
                    vec![("T".to_string(), TypeExpr::Wildcard)],
                    Box::new(TypeExpr::Map(vec![
                        ("k1".to_string(), TypeExpr::Named("T".to_string())),
                        ("k2".to_string(), TypeExpr::Named("T".to_string())),
                    ])),
                ))
            )]
        );
    }

    #[test]
    fn test_generic_multiple_params() {
        assert_eq!(
            parse("multi: '<T i64, U _> {k1: T, k2: U}").unwrap(),
            vec![def(
                "multi",
                Expr::StructuralType(TypeExpr::Generic(
                    vec![
                        ("T".to_string(), TypeExpr::Named("i64".to_string())),
                        ("U".to_string(), TypeExpr::Wildcard),
                    ],
                    Box::new(TypeExpr::Map(vec![
                        ("k1".to_string(), TypeExpr::Named("T".to_string())),
                        ("k2".to_string(), TypeExpr::Named("U".to_string())),
                    ])),
                ))
            )]
        );
    }

    #[test]
    fn test_generic_nested_constraint() {
        assert_eq!(
            parse("nested: '<T i64, U <V _>{k1: V}> {k1: T}").unwrap(),
            vec![def(
                "nested",
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
                ))
            )]
        );
    }

    #[test]
    fn test_generic_unclosed_angle_bracket_error() {
        assert_eq!(
            parse("f: '<T _").unwrap_err().kind,
            ParseErrorKind::InvalidTypeExpr
        );
    }
}
