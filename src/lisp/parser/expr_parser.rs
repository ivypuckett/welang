use chumsky::prelude::*;

use crate::lisp::lexer::Token;
use crate::lisp::parser::type_parser::type_parser;
use crate::lisp::parser::types::{Expr, ParseError, ParseErrorKind, TypeExpr};

pub(crate) fn expr_parser() -> impl Parser<Token, Expr, Error = ParseError> + Clone {
    recursive(|expr| {
        let atom = select! {
            Token::Number(n) => Expr::Number(n),
            Token::Bool(b) => Expr::Bool(b),
            Token::Str(s) => Expr::Str(s),
            Token::Symbol(s) => Expr::Symbol(s),
        };

        let structural = just(Token::Quote)
            .ignore_then(type_parser().labelled(ParseErrorKind::InvalidTypeExpr).or(
                end().validate(|(), span: std::ops::Range<usize>, emit| {
                    emit(ParseError {
                        kind: ParseErrorKind::MissingQuoteTarget,
                        line: span.start,
                    });
                    TypeExpr::Wildcard
                }),
            ))
            .map(Expr::StructuralType);

        let nominal = just(Token::Star)
            .ignore_then(type_parser().labelled(ParseErrorKind::MissingQuoteTarget))
            .map(Expr::NominalType);

        let tuple = expr
            .clone()
            .separated_by(just(Token::Comma))
            .allow_trailing()
            .delimited_by(
                just(Token::LBracket),
                just(Token::RBracket).labelled(ParseErrorKind::UnmatchedOpenBracket),
            )
            .map(Expr::Tuple);

        // Rename: (sym: body)
        let rename = select! { Token::Symbol(s) => s }
            .then_ignore(just(Token::Colon))
            .then(expr.clone())
            .delimited_by(
                just(Token::LParen),
                just(Token::RParen).labelled(ParseErrorKind::UnmatchedOpenParen),
            )
            .map(|(name, body)| Expr::Rename(name, Box::new(body)));

        // Pipe/list: (e e | e e | ...)
        let segment = expr.clone().repeated();
        let list_or_pipe = segment
            .clone()
            .then(just(Token::Pipe).ignore_then(segment.clone()).repeated())
            .delimited_by(
                just(Token::LParen),
                just(Token::RParen).labelled(ParseErrorKind::UnmatchedOpenParen),
            )
            .validate(
                |(first, rest): (Vec<Expr>, Vec<Vec<Expr>>), span: std::ops::Range<usize>, emit| {
                    if first.is_empty() || rest.iter().any(|s| s.is_empty()) {
                        emit(ParseError {
                            kind: ParseErrorKind::EmptyPipeSegment,
                            line: span.start,
                        });
                    }
                    (first, rest)
                },
            )
            .map(|(first, rest)| {
                if rest.is_empty() {
                    Expr::List(first)
                } else {
                    let mut acc = fold_pipe(first, None);
                    for seg in rest {
                        acc = fold_pipe(seg, Some(acc));
                    }
                    acc
                }
            });

        // .or() in chumsky 0.9 already backtracks — no .rewind() needed
        let paren = rename.or(list_or_pipe);

        // Cond key uses the full paren parser so (x) → List([Symbol("x")])
        let cond_key_paren = paren.clone().map(Some);
        let cond_key_wild = just(Token::Symbol("_".to_string())).to(None);
        let cond_entry = cond_key_paren
            .or(cond_key_wild)
            .then_ignore(just(Token::Colon).labelled(ParseErrorKind::InvalidCondEntry))
            .then(expr.clone());

        let cond = cond_entry
            .separated_by(just(Token::Comma))
            .allow_trailing()
            .delimited_by(
                just(Token::LBrace),
                just(Token::RBrace).labelled(ParseErrorKind::UnmatchedOpenBrace),
            )
            .validate(
                |entries: Vec<(Option<Expr>, Expr)>, span: std::ops::Range<usize>, emit| {
                    let wild = entries.iter().position(|(k, _)| k.is_none());
                    match wild {
                        Some(i) if i != entries.len() - 1 => emit(ParseError {
                            kind: ParseErrorKind::MisplacedCondWildcard,
                            line: span.start,
                        }),
                        None => emit(ParseError {
                            kind: ParseErrorKind::MissingCondWildcard,
                            line: span.start,
                        }),
                        _ => {}
                    }
                    entries
                },
            )
            .map(Expr::Cond);

        let map_entry = select! { Token::Symbol(s) if s != "_" => s }
            .then_ignore(just(Token::Colon).labelled(ParseErrorKind::InvalidMapEntry))
            .then(expr.clone());

        let map = map_entry
            .separated_by(just(Token::Comma))
            .allow_trailing()
            .delimited_by(
                just(Token::LBrace),
                just(Token::RBrace).labelled(ParseErrorKind::UnmatchedOpenBrace),
            )
            .map(Expr::Map);

        let brace = cond.or(map);

        let primary = choice((paren, tuple, brace, structural, nominal, atom));

        primary
            .then(
                just(Token::Dot)
                    .ignore_then(
                        select! { Token::Symbol(s) => s }
                            .labelled(ParseErrorKind::InvalidDotAccess),
                    )
                    .then(
                        select! {
                            Token::Number(n) => Expr::Number(n),
                            Token::Bool(b) => Expr::Bool(b),
                            Token::Str(s) => Expr::Str(s),
                            Token::Symbol(s) => Expr::Symbol(s),
                        }
                        .or_not(),
                    )
                    .repeated(),
            )
            .map(|(base, dots): (Expr, Vec<(String, Option<Expr>)>)| {
                dots.into_iter().fold(base, |acc, (name, rhs)| match rhs {
                    Some(arg) => Expr::List(vec![Expr::Symbol(name), Expr::Tuple(vec![acc, arg])]),
                    None => Expr::List(vec![
                        Expr::Symbol("get".to_string()),
                        Expr::Tuple(vec![acc, Expr::Symbol(name)]),
                    ]),
                })
            })
    })
}

fn fold_pipe(items: Vec<Expr>, seed: Option<Expr>) -> Expr {
    let mut iter = items.into_iter().rev();
    let last = iter.next().unwrap_or(Expr::List(vec![]));
    let mut acc = match seed {
        None => last,
        Some(arg) => Expr::List(vec![last, arg]),
    };
    for func in iter {
        acc = Expr::List(vec![func, acc]);
    }
    acc
}

#[cfg(test)]
mod tests {
    use crate::lisp::parser::parse;
    use crate::lisp::parser::types::{Expr, ParseErrorKind};

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
