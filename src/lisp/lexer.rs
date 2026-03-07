/// A token produced by the LISP lexer.
#[derive(Debug, PartialEq, Clone)]
pub enum Token {
    LParen,
    RParen,
    Quote,
    Bool(bool),
    Number(f64),
    Str(String),
    Symbol(String),
}

/// Errors that can occur during lexing.
#[derive(Debug, PartialEq)]
pub enum LexError {
    UnterminatedString,
    InvalidNumber(String),
}

impl std::fmt::Display for LexError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LexError::UnterminatedString => write!(f, "unterminated string literal"),
            LexError::InvalidNumber(s) => write!(f, "invalid number: {s}"),
        }
    }
}

/// Tokenize a LISP source string into a list of tokens.
pub fn tokenize(input: &str) -> Result<Vec<Token>, LexError> {
    let mut tokens = Vec::new();
    let mut chars = input.chars().peekable();

    while let Some(&c) = chars.peek() {
        match c {
            // Whitespace
            ' ' | '\t' | '\n' | '\r' => {
                chars.next();
            }

            // Line comments
            ';' => {
                while let Some(&ch) = chars.peek() {
                    chars.next();
                    if ch == '\n' {
                        break;
                    }
                }
            }

            '(' => {
                chars.next();
                tokens.push(Token::LParen);
            }

            ')' => {
                chars.next();
                tokens.push(Token::RParen);
            }

            '\'' => {
                chars.next();
                tokens.push(Token::Quote);
            }

            // String literals
            '"' => {
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
                    return Err(LexError::UnterminatedString);
                }
                tokens.push(Token::Str(s));
            }

            // Boolean literals: #t, #f
            '#' => {
                chars.next();
                match chars.peek() {
                    Some(&'t') => {
                        chars.next();
                        tokens.push(Token::Bool(true));
                    }
                    Some(&'f') => {
                        chars.next();
                        tokens.push(Token::Bool(false));
                    }
                    _ => {
                        tokens.push(Token::Symbol("#".to_string()));
                    }
                }
            }

            // Numbers and symbols
            _ => {
                let mut word = String::new();
                while let Some(&ch) = chars.peek() {
                    if ch.is_whitespace() || ch == '(' || ch == ')' || ch == '"' || ch == ';' {
                        break;
                    }
                    chars.next();
                    word.push(ch);
                }

                // Try to parse as a number first
                if looks_like_number(&word) {
                    match word.parse::<f64>() {
                        Ok(n) => tokens.push(Token::Number(n)),
                        Err(_) => return Err(LexError::InvalidNumber(word)),
                    }
                } else {
                    tokens.push(Token::Symbol(word));
                }
            }
        }
    }

    Ok(tokens)
}

/// Returns true if the word should be parsed as a number.
fn looks_like_number(word: &str) -> bool {
    let s = word.strip_prefix('-').unwrap_or(word);
    if s.is_empty() {
        return false;
    }
    let s = s.strip_prefix('+').unwrap_or(s);
    if s.is_empty() {
        return false;
    }
    s.chars()
        .next()
        .map_or(false, |c| c.is_ascii_digit() || c == '.')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_input() {
        assert_eq!(tokenize("").unwrap(), vec![]);
    }

    #[test]
    fn test_parens() {
        assert_eq!(tokenize("()").unwrap(), vec![Token::LParen, Token::RParen]);
    }

    #[test]
    fn test_nested_parens() {
        assert_eq!(
            tokenize("(())").unwrap(),
            vec![Token::LParen, Token::LParen, Token::RParen, Token::RParen]
        );
    }

    #[test]
    fn test_integer() {
        assert_eq!(tokenize("42").unwrap(), vec![Token::Number(42.0)]);
    }

    #[test]
    fn test_negative_number() {
        assert_eq!(tokenize("-3").unwrap(), vec![Token::Number(-3.0)]);
    }

    #[test]
    fn test_float() {
        assert_eq!(tokenize("3.14").unwrap(), vec![Token::Number(3.14)]);
    }

    #[test]
    fn test_symbol() {
        assert_eq!(
            tokenize("foo").unwrap(),
            vec![Token::Symbol("foo".to_string())]
        );
    }

    #[test]
    fn test_operator_symbols() {
        assert_eq!(
            tokenize("+ - * /").unwrap(),
            vec![
                Token::Symbol("+".to_string()),
                Token::Symbol("-".to_string()),
                Token::Symbol("*".to_string()),
                Token::Symbol("/".to_string()),
            ]
        );
    }

    #[test]
    fn test_bool_true() {
        assert_eq!(tokenize("#t").unwrap(), vec![Token::Bool(true)]);
    }

    #[test]
    fn test_bool_false() {
        assert_eq!(tokenize("#f").unwrap(), vec![Token::Bool(false)]);
    }

    #[test]
    fn test_string() {
        assert_eq!(
            tokenize(r#""hello""#).unwrap(),
            vec![Token::Str("hello".to_string())]
        );
    }

    #[test]
    fn test_string_with_escapes() {
        assert_eq!(
            tokenize(r#""line1\nline2""#).unwrap(),
            vec![Token::Str("line1\nline2".to_string())]
        );
    }

    #[test]
    fn test_unterminated_string() {
        assert_eq!(
            tokenize(r#""oops"#).unwrap_err(),
            LexError::UnterminatedString
        );
    }

    #[test]
    fn test_quote_shorthand() {
        assert_eq!(
            tokenize("'x").unwrap(),
            vec![Token::Quote, Token::Symbol("x".to_string())]
        );
    }

    #[test]
    fn test_line_comment() {
        assert_eq!(
            tokenize("; this is a comment\n42").unwrap(),
            vec![Token::Number(42.0)]
        );
    }

    #[test]
    fn test_inline_comment() {
        assert_eq!(
            tokenize("(+ 1 2) ; add").unwrap(),
            vec![
                Token::LParen,
                Token::Symbol("+".to_string()),
                Token::Number(1.0),
                Token::Number(2.0),
                Token::RParen,
            ]
        );
    }

    #[test]
    fn test_whitespace_handling() {
        assert_eq!(
            tokenize("  ( +  1   2 )  ").unwrap(),
            vec![
                Token::LParen,
                Token::Symbol("+".to_string()),
                Token::Number(1.0),
                Token::Number(2.0),
                Token::RParen,
            ]
        );
    }

    #[test]
    fn test_define_expression() {
        assert_eq!(
            tokenize("(define x 10)").unwrap(),
            vec![
                Token::LParen,
                Token::Symbol("define".to_string()),
                Token::Symbol("x".to_string()),
                Token::Number(10.0),
                Token::RParen,
            ]
        );
    }

    #[test]
    fn test_lambda_expression() {
        assert_eq!(
            tokenize("(lambda (x) x)").unwrap(),
            vec![
                Token::LParen,
                Token::Symbol("lambda".to_string()),
                Token::LParen,
                Token::Symbol("x".to_string()),
                Token::RParen,
                Token::Symbol("x".to_string()),
                Token::RParen,
            ]
        );
    }

    #[test]
    fn test_multiple_expressions() {
        assert_eq!(
            tokenize("(+ 1 2)\n(* 3 4)").unwrap(),
            vec![
                Token::LParen,
                Token::Symbol("+".to_string()),
                Token::Number(1.0),
                Token::Number(2.0),
                Token::RParen,
                Token::LParen,
                Token::Symbol("*".to_string()),
                Token::Number(3.0),
                Token::Number(4.0),
                Token::RParen,
            ]
        );
    }
}
