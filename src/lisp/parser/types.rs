use crate::lisp::lexer::LexError;

#[derive(Debug, PartialEq, Clone)]
pub enum TypeExpr {
    Named(String),
    Wildcard,
    Array(Box<TypeExpr>),
    Map(Vec<(String, TypeExpr)>),
    Function(Box<TypeExpr>, Box<TypeExpr>),
    Generic(Vec<(String, TypeExpr)>, Box<TypeExpr>),
    Nominal(Box<TypeExpr>),
}

#[derive(Debug, PartialEq, Clone)]
pub enum Expr {
    Number(f64),
    Bool(bool),
    Str(String),
    Symbol(String),
    StructuralType(TypeExpr),
    NominalType(TypeExpr),
    List(Vec<Expr>),
    Tuple(Vec<Expr>),
    Map(Vec<(String, Expr)>),
    Cond(Vec<(Option<Expr>, Expr)>),
    Rename(String, Box<Expr>),
}

#[derive(Debug, PartialEq)]
pub enum ParseErrorKind {
    Lex(LexError),
    UnmatchedOpenParen,
    UnmatchedOpenBracket,
    UnexpectedCloseParen,
    UnexpectedCloseBracket,
    MissingQuoteTarget,
    InvalidFuncDef,
    UnexpectedTopLevel,
    UnexpectedPipe,
    EmptyPipeSegment,
    UnmatchedOpenBrace,
    UnexpectedCloseBrace,
    InvalidMapEntry,
    InvalidCondEntry,
    MissingCondWildcard,
    MisplacedCondWildcard,
    UnexpectedDot,
    InvalidDotAccess,
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
            ParseErrorKind::EmptyPipeSegment => write!(
                f,
                "empty pipe segment: '|' requires an expression on each side"
            ),
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
            ParseErrorKind::MisplacedCondWildcard => {
                write!(
                    f,
                    "the '_' wildcard must be the last entry in a conditional"
                )
            }
            ParseErrorKind::UnexpectedDot => {
                write!(f, "unexpected '.' — dot access must follow an expression")
            }
            ParseErrorKind::InvalidDotAccess => {
                write!(f, "expected a field or method name after '.'")
            }
            ParseErrorKind::InvalidTypeExpr => write!(
                f,
                "invalid type expression after ''' — expected a type name, '[T]', '{{k: T}}', '(A | B)', or '<T C> body'"
            ),
        }
    }
}

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
