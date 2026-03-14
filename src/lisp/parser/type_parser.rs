use chumsky::prelude::*;

use crate::lisp::lexer::Token;
use crate::lisp::parser::types::{ParseError, ParseErrorKind, TypeExpr};

pub(crate) fn type_parser() -> impl Parser<Token, TypeExpr, Error = ParseError> + Clone {
    recursive(|ty| {
        let named = select! {
            Token::Symbol(s) if s == "_" => TypeExpr::Wildcard,
            Token::Symbol(s) => TypeExpr::Named(s),
        };

        let array = ty
            .clone()
            .delimited_by(
                just(Token::LBracket),
                just(Token::RBracket).labelled(ParseErrorKind::UnmatchedOpenBracket),
            )
            .map(|inner| TypeExpr::Array(Box::new(inner)));

        let map_entry = select! { Token::Symbol(s) => s }
            .then_ignore(just(Token::Colon).labelled(ParseErrorKind::InvalidMapEntry))
            .then(ty.clone());

        let map = map_entry
            .clone()
            .separated_by(just(Token::Comma))
            .allow_trailing()
            .delimited_by(
                just(Token::LBrace),
                just(Token::RBrace).labelled(ParseErrorKind::UnmatchedOpenBrace),
            )
            .map(TypeExpr::Map);

        let func_or_paren = ty
            .clone()
            .then(just(Token::Pipe).ignore_then(ty.clone()).or_not())
            .delimited_by(
                just(Token::LParen),
                just(Token::RParen).labelled(ParseErrorKind::UnmatchedOpenParen),
            )
            .map(|(inp, out)| match out {
                Some(o) => TypeExpr::Function(Box::new(inp), Box::new(o)),
                None => inp,
            });

        let generic_param = select! { Token::Symbol(s) => s }.then(ty.clone());

        let generic = generic_param
            .separated_by(just(Token::Comma))
            .allow_trailing()
            .delimited_by(
                just(Token::LAngle),
                just(Token::RAngle).labelled(ParseErrorKind::InvalidTypeExpr),
            )
            .then(ty.clone())
            .map(|(params, body)| TypeExpr::Generic(params, Box::new(body)));

        let nominal = just(Token::Star)
            .ignore_then(ty.clone())
            .map(|inner| TypeExpr::Nominal(Box::new(inner)));

        choice((generic, array, func_or_paren, map, nominal, named))
    })
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
