mod expr_parser;
mod type_parser;
pub mod types;

pub use types::{Expr, ParseError, ParseErrorKind, TypeExpr};

use chumsky::Stream;
use chumsky::prelude::*;

use crate::lisp::lexer::{Token, tokenize};
use expr_parser::expr_parser;
use type_parser::type_parser;

fn func_def_parser() -> impl Parser<Token, Expr, Error = ParseError> {
    let name = select! { Token::Symbol(s) => s };

    // annotation + colon, all in one unit
    let no_ann = just(Token::Colon).to(None::<Expr>);

    let structural_ann = just(Token::Quote)
        .ignore_then(type_parser())
        .then_ignore(just(Token::Colon).labelled(ParseErrorKind::InvalidFuncDef))
        .map(|ty| Some(Expr::StructuralType(ty)));

    // Named annotation: sym followed by colon (e.g. `foo anyFloat: body`)
    let named_ann = select! { Token::Symbol(s) if s != "_" => s }
        .then_ignore(just(Token::Colon))
        .map(|s| Some(Expr::Symbol(s)));

    // Generic annotation: must start with < (e.g. `f <T _> (T|T): body`)
    let generic_ann = just(Token::LAngle)
        .rewind()
        .ignore_then(type_parser())
        .then_ignore(just(Token::Colon).labelled(ParseErrorKind::InvalidFuncDef))
        .map(|ty| Some(Expr::StructuralType(ty)));

    let ann_colon = choice((no_ann, structural_ann, named_ann, generic_ann))
        .labelled(ParseErrorKind::InvalidFuncDef);

    name.then(ann_colon)
        .then(expr_parser())
        .map(|((name, ann), body)| {
            let sig = Expr::List(vec![Expr::Symbol(name), Expr::Symbol("x".to_string())]);
            let mut items = vec![Expr::Symbol("define".to_string()), sig, body];
            if let Some(a) = ann {
                items.push(a);
            }
            Expr::List(items)
        })
}

pub fn parse(input: &str) -> Result<Vec<Expr>, ParseError> {
    let tokens = tokenize(input)?;

    let eoi_line = tokens.last().map(|(_, l)| *l).unwrap_or(1);
    let stream = Stream::from_iter(
        eoi_line..eoi_line + 1,
        tokens
            .iter()
            .map(|(tok, line)| (tok.clone(), *line..*line + 1)),
    );

    func_def_parser()
        .repeated()
        .then_ignore(end().labelled(ParseErrorKind::UnexpectedTopLevel))
        .parse(stream)
        .map_err(|errors| {
            errors.into_iter().next().unwrap_or(ParseError {
                kind: ParseErrorKind::InvalidFuncDef,
                line: eoi_line,
            })
        })
}

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn test_bare_number_is_error() {
        assert_eq!(
            parse("42").unwrap_err().kind,
            ParseErrorKind::UnexpectedTopLevel
        );
    }

    #[test]
    fn test_bare_expr_is_error() {
        assert_eq!(
            parse("(+ 1 2)").unwrap_err().kind,
            ParseErrorKind::UnexpectedTopLevel
        );
    }

    #[test]
    fn test_multi_body_is_error() {
        assert_eq!(
            parse("f: 1 2").unwrap_err().kind,
            ParseErrorKind::UnexpectedTopLevel
        );
    }

    #[test]
    fn test_expr_after_func_def_is_error() {
        assert_eq!(
            parse("main: 0\n42").unwrap_err().kind,
            ParseErrorKind::UnexpectedTopLevel
        );
    }

    #[test]
    fn test_func_def() {
        assert_eq!(
            parse("main: 0").unwrap(),
            vec![def("main", Expr::Number(0.0))]
        );
    }

    #[test]
    fn test_one_arg_func() {
        assert_eq!(
            parse("double: (multiply [2, x])").unwrap(),
            vec![def(
                "double",
                Expr::List(vec![
                    sym("multiply"),
                    Expr::Tuple(vec![Expr::Number(2.0), sym("x")]),
                ])
            )]
        );
    }

    #[test]
    fn test_multiple_func_defs() {
        let result = parse("foo: 1\nbar: (multiply [2, x])").unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(result[0], def("foo", Expr::Number(1.0)));
        assert_eq!(
            result[1],
            def(
                "bar",
                Expr::List(vec![
                    sym("multiply"),
                    Expr::Tuple(vec![Expr::Number(2.0), sym("x")]),
                ])
            )
        );
    }

    #[test]
    fn test_rename_expr() {
        assert_eq!(
            parse("id: (n: n)").unwrap(),
            vec![def("id", Expr::Rename("n".to_string(), Box::new(sym("n"))))]
        );
    }

    #[test]
    fn test_tuple_parse() {
        assert_eq!(
            parse("f: [1, 2]").unwrap(),
            vec![def(
                "f",
                Expr::Tuple(vec![Expr::Number(1.0), Expr::Number(2.0)])
            )]
        );
    }

    #[test]
    fn test_missing_body() {
        assert_eq!(
            parse("foo:").unwrap_err().kind,
            ParseErrorKind::InvalidFuncDef
        );
    }

    #[test]
    fn test_missing_body_before_next_def() {
        assert_eq!(
            parse("foo:\nbar: 1").unwrap_err().kind,
            ParseErrorKind::UnexpectedTopLevel
        );
    }

    #[test]
    fn test_unmatched_open_paren() {
        assert_eq!(
            parse("f: (+ 1 2").unwrap_err().kind,
            ParseErrorKind::UnmatchedOpenParen
        );
    }

    #[test]
    fn test_unmatched_open_bracket() {
        assert_eq!(
            parse("f: [1, 2").unwrap_err().kind,
            ParseErrorKind::UnmatchedOpenBracket
        );
    }

    #[test]
    fn test_missing_quote_target() {
        assert_eq!(
            parse("f: '").unwrap_err().kind,
            ParseErrorKind::MissingQuoteTarget
        );
    }

    #[test]
    fn test_lex_error_propagated() {
        assert!(matches!(
            parse(r#"f: "unterminated"#).unwrap_err().kind,
            ParseErrorKind::Lex(_)
        ));
    }

    #[test]
    fn test_comment_ignored() {
        assert_eq!(
            parse("# ignore\nmain: 0").unwrap(),
            vec![def("main", Expr::Number(0.0))]
        );
    }

    #[test]
    fn test_error_line_number_first_line() {
        assert_eq!(parse("42").unwrap_err().line, 1);
    }

    #[test]
    fn test_error_line_number_second_line() {
        assert_eq!(parse("main: 0\n42").unwrap_err().line, 2);
    }

    #[test]
    fn test_error_line_unmatched_paren() {
        let err = parse("main: 0\nf: (+ 1 2").unwrap_err();
        assert_eq!(err.line, 2);
        assert_eq!(err.kind, ParseErrorKind::UnmatchedOpenParen);
    }

    #[test]
    fn test_lex_error_line_propagated() {
        assert_eq!(parse("a: 1\nb: 2\nc: \"oops").unwrap_err().line, 3);
    }

    #[test]
    fn test_decimal_literal_in_func_body() {
        assert_eq!(
            parse("f: 3f14").unwrap(),
            vec![def("f", Expr::Number(3.14))]
        );
    }

    #[test]
    fn test_negative_decimal_literal_in_func_body() {
        assert_eq!(
            parse("f: -1f5").unwrap(),
            vec![def("f", Expr::Number(-1.5))]
        );
    }

    #[test]
    fn test_decimal_in_tuple() {
        assert_eq!(
            parse("f: [1f5, 2f5]").unwrap(),
            vec![def(
                "f",
                Expr::Tuple(vec![Expr::Number(1.5), Expr::Number(2.5)])
            )]
        );
    }

    #[test]
    fn test_cond_simple() {
        assert_eq!(
            parse("f: {(x): 1, _: 0}").unwrap(),
            vec![def(
                "f",
                Expr::Cond(vec![
                    (Some(Expr::List(vec![sym("x")])), Expr::Number(1.0)),
                    (None, Expr::Number(0.0)),
                ])
            )]
        );
    }

    #[test]
    fn test_cond_multi_arm() {
        assert_eq!(
            parse("f: {(lessThan [x, 0]): 1, (greaterThan [x, 0]): 2, _: 0}").unwrap(),
            vec![def(
                "f",
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
                ])
            )]
        );
    }

    #[test]
    fn test_cond_missing_wildcard_error() {
        assert_eq!(
            parse("f: {(x): 1}").unwrap_err().kind,
            ParseErrorKind::MissingCondWildcard
        );
    }

    #[test]
    fn test_cond_misplaced_wildcard_error() {
        assert_eq!(
            parse("f: {_: 0, (x): 1}").unwrap_err().kind,
            ParseErrorKind::MisplacedCondWildcard
        );
    }

    #[test]
    fn test_cond_wildcard_only() {
        assert_eq!(
            parse("f: {_: 42}").unwrap(),
            vec![def("f", Expr::Cond(vec![(None, Expr::Number(42.0))]))]
        );
    }
}
