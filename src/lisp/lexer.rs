/// A token produced by the LISP lexer.
#[derive(Debug, PartialEq, Clone)]
pub enum Token {
    LParen,
    RParen,
    LBracket,
    RBracket,
    LBrace,
    RBrace,
    Comma,
    Colon,
    Pipe,
    Quote,
    Bool(bool),
    Number(f64),
    Str(String),
    Symbol(String),
}

/// The kind of error that occurred during lexing.
#[derive(Debug, PartialEq)]
pub enum LexErrorKind {
    UnterminatedString,
    InvalidNumber(String),
}

impl std::fmt::Display for LexErrorKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LexErrorKind::UnterminatedString => write!(f, "unterminated string literal"),
            LexErrorKind::InvalidNumber(s) => write!(f, "invalid number: {s}"),
        }
    }
}

/// An error that occurred during lexing, with a 1-indexed line number.
#[derive(Debug, PartialEq)]
pub struct LexError {
    pub kind: LexErrorKind,
    pub line: usize,
}

impl std::fmt::Display for LexError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.kind)
    }
}

/// Tokenize a LISP source string into a list of `(token, line)` pairs.
/// Line numbers are 1-indexed.
pub fn tokenize(input: &str) -> Result<Vec<(Token, usize)>, LexError> {
    let mut tokens = Vec::new();
    let mut chars = input.chars().peekable();
    let mut line: usize = 1;

    while let Some(&c) = chars.peek() {
        match c {
            ' ' | '\t' | '\r' => {
                chars.next();
            }

            '\n' => {
                chars.next();
                line += 1;
            }

            '#' => {
                while let Some(&ch) = chars.peek() {
                    chars.next();
                    if ch == '\n' {
                        line += 1;
                        break;
                    }
                }
            }

            '(' => {
                chars.next();
                tokens.push((Token::LParen, line));
            }
            ')' => {
                chars.next();
                tokens.push((Token::RParen, line));
            }
            '[' => {
                chars.next();
                tokens.push((Token::LBracket, line));
            }
            ']' => {
                chars.next();
                tokens.push((Token::RBracket, line));
            }
            '{' => {
                chars.next();
                tokens.push((Token::LBrace, line));
            }
            '}' => {
                chars.next();
                tokens.push((Token::RBrace, line));
            }
            ',' => {
                chars.next();
                tokens.push((Token::Comma, line));
            }
            ':' => {
                chars.next();
                tokens.push((Token::Colon, line));
            }
            '|' => {
                chars.next();
                tokens.push((Token::Pipe, line));
            }
            '\'' => {
                chars.next();
                tokens.push((Token::Quote, line));
            }

            '"' => {
                let string_line = line;
                chars.next();
                let mut s = String::new();
                let mut closed = false;
                while let Some(&ch) = chars.peek() {
                    chars.next();
                    match ch {
                        '"' => {
                            closed = true;
                            break;
                        }
                        '\n' => {
                            line += 1;
                            s.push('\n');
                        }
                        '\\' => match chars.peek() {
                            Some(&'n') => {
                                chars.next();
                                s.push('\n');
                            }
                            Some(&'t') => {
                                chars.next();
                                s.push('\t');
                            }
                            Some(&'r') => {
                                chars.next();
                                s.push('\r');
                            }
                            Some(&'"') => {
                                chars.next();
                                s.push('"');
                            }
                            Some(&'\\') => {
                                chars.next();
                                s.push('\\');
                            }
                            _ => {
                                s.push('\\');
                            }
                        },
                        _ => s.push(ch),
                    }
                }
                if !closed {
                    return Err(LexError {
                        kind: LexErrorKind::UnterminatedString,
                        line: string_line,
                    });
                }
                tokens.push((Token::Str(s), string_line));
            }

            _ => {
                let word_line = line;
                let mut word = String::new();
                while let Some(&ch) = chars.peek() {
                    if ch.is_whitespace()
                        || ch == '('
                        || ch == ')'
                        || ch == '['
                        || ch == ']'
                        || ch == '{'
                        || ch == '}'
                        || ch == ','
                        || ch == '"'
                        || ch == '#'
                        || ch == ':'
                        || ch == '|'
                    {
                        break;
                    }
                    chars.next();
                    word.push(ch);
                }
                if word == "true" {
                    tokens.push((Token::Bool(true), word_line));
                } else if word == "false" {
                    tokens.push((Token::Bool(false), word_line));
                } else if looks_like_number(&word) {
                    let normalized = word.replacen('f', ".", 1);
                    match normalized.parse::<f64>() {
                        Ok(n) => tokens.push((Token::Number(n), word_line)),
                        Err(_) => {
                            return Err(LexError {
                                kind: LexErrorKind::InvalidNumber(word),
                                line: word_line,
                            });
                        }
                    }
                } else {
                    tokens.push((Token::Symbol(word), word_line));
                }
            }
        }
    }

    Ok(tokens)
}

fn looks_like_number(word: &str) -> bool {
    let s = word.strip_prefix('-').unwrap_or(word);
    if s.is_empty() {
        return false;
    }
    let s = s.strip_prefix('+').unwrap_or(s);
    if s.is_empty() {
        return false;
    }
    s.chars().next().is_some_and(|c| c.is_ascii_digit())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Strip line numbers from tokenize output for brevity in most tests.
    fn tok(input: &str) -> Vec<Token> {
        tokenize(input)
            .unwrap()
            .into_iter()
            .map(|(t, _)| t)
            .collect()
    }

    fn tok_err(input: &str) -> LexErrorKind {
        tokenize(input).unwrap_err().kind
    }

    #[test]
    fn test_empty_input() {
        assert_eq!(tok(""), vec![]);
    }

    #[test]
    fn test_parens() {
        assert_eq!(tok("()"), vec![Token::LParen, Token::RParen]);
    }

    #[test]
    fn test_integer() {
        assert_eq!(tok("42"), vec![Token::Number(42.0)]);
    }

    #[test]
    fn test_negative_number() {
        assert_eq!(tok("-3"), vec![Token::Number(-3.0)]);
    }

    #[test]
    fn test_symbol() {
        assert_eq!(tok("foo"), vec![Token::Symbol("foo".to_string())]);
    }

    #[test]
    fn test_operator_symbols() {
        assert_eq!(
            tok("add subtract multiply divide"),
            vec![
                Token::Symbol("add".to_string()),
                Token::Symbol("subtract".to_string()),
                Token::Symbol("multiply".to_string()),
                Token::Symbol("divide".to_string()),
            ]
        );
    }

    #[test]
    fn test_bool_true() {
        assert_eq!(tok("true"), vec![Token::Bool(true)]);
    }

    #[test]
    fn test_bool_false() {
        assert_eq!(tok("false"), vec![Token::Bool(false)]);
    }

    #[test]
    fn test_string() {
        assert_eq!(tok(r#""hello""#), vec![Token::Str("hello".to_string())]);
    }

    #[test]
    fn test_unterminated_string() {
        assert_eq!(tok_err(r#""oops"#), LexErrorKind::UnterminatedString);
    }

    #[test]
    fn test_unterminated_string_line() {
        let err = tokenize("foo\n\"oops").unwrap_err();
        assert_eq!(err.line, 2);
    }

    #[test]
    fn test_line_comment() {
        assert_eq!(tok("# comment\n42"), vec![Token::Number(42.0)]);
    }

    #[test]
    fn test_colon() {
        assert_eq!(
            tok("foo:"),
            vec![Token::Symbol("foo".to_string()), Token::Colon]
        );
    }

    #[test]
    fn test_tuple_tokens() {
        assert_eq!(
            tok("[1, 2]"),
            vec![
                Token::LBracket,
                Token::Number(1.0),
                Token::Comma,
                Token::Number(2.0),
                Token::RBracket,
            ]
        );
    }

    #[test]
    fn test_func_def_tokens() {
        assert_eq!(
            tok("double: (multiply [2, x])"),
            vec![
                Token::Symbol("double".to_string()),
                Token::Colon,
                Token::LParen,
                Token::Symbol("multiply".to_string()),
                Token::LBracket,
                Token::Number(2.0),
                Token::Comma,
                Token::Symbol("x".to_string()),
                Token::RBracket,
                Token::RParen,
            ]
        );
    }

    #[test]
    fn test_decimal_number() {
        assert_eq!(tok("3f14"), vec![Token::Number(3.14)]);
    }

    #[test]
    fn test_negative_decimal_number() {
        assert_eq!(tok("-1f5"), vec![Token::Number(-1.5)]);
    }

    #[test]
    fn test_line_numbers() {
        let tokens = tokenize("foo\nbar\nbaz").unwrap();
        let lines: Vec<usize> = tokens.iter().map(|(_, l)| *l).collect();
        assert_eq!(lines, vec![1, 2, 3]);
    }
}
