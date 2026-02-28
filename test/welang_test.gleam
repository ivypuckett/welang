import gleam/result
import gleeunit
import gleeunit/should
import lexer

pub fn main() {
  gleeunit.main()
}

// ---------------------------------------------------------------------------
// Numeric literals
// ---------------------------------------------------------------------------

pub fn lex_integer_test() {
  lexer.lex("42")
  |> should.equal(Ok([lexer.TokInt("42"), lexer.TokEof]))
}

pub fn lex_negative_integer_test() {
  lexer.lex("-5")
  |> should.equal(Ok([lexer.TokInt("-5"), lexer.TokEof]))
}

pub fn lex_zero_test() {
  lexer.lex("0")
  |> should.equal(Ok([lexer.TokInt("0"), lexer.TokEof]))
}

pub fn lex_float_test() {
  lexer.lex("3.14")
  |> should.equal(Ok([lexer.TokFloat("3.14"), lexer.TokEof]))
}

pub fn lex_negative_float_test() {
  lexer.lex("-2.0")
  |> should.equal(Ok([lexer.TokFloat("-2.0"), lexer.TokEof]))
}

pub fn lex_integer_not_float_test() {
  // `5.` is an integer followed by a dot, not a float
  lexer.lex("5.")
  |> should.equal(Ok([lexer.TokInt("5"), lexer.TokDot, lexer.TokEof]))
}

// ---------------------------------------------------------------------------
// String literals
// ---------------------------------------------------------------------------

pub fn lex_string_test() {
  lexer.lex("\"hello\"")
  |> should.equal(Ok([lexer.TokString("hello"), lexer.TokEof]))
}

pub fn lex_empty_string_test() {
  lexer.lex("\"\"")
  |> should.equal(Ok([lexer.TokString(""), lexer.TokEof]))
}

pub fn lex_string_escape_newline_test() {
  lexer.lex("\"a\\nb\"")
  |> should.equal(Ok([lexer.TokString("a\nb"), lexer.TokEof]))
}

pub fn lex_string_escape_tab_test() {
  lexer.lex("\"a\\tb\"")
  |> should.equal(Ok([lexer.TokString("a\tb"), lexer.TokEof]))
}

pub fn lex_string_escape_quote_test() {
  lexer.lex("\"say \\\"hi\\\"\"")
  |> should.equal(Ok([lexer.TokString("say \"hi\""), lexer.TokEof]))
}

pub fn lex_string_escape_backslash_test() {
  lexer.lex("\"a\\\\b\"")
  |> should.equal(Ok([lexer.TokString("a\\b"), lexer.TokEof]))
}

// ---------------------------------------------------------------------------
// Interpolated strings
// ---------------------------------------------------------------------------

pub fn lex_interpolated_plain_test() {
  lexer.lex("`hello`")
  |> should.equal(Ok([
    lexer.TokInterpolated([lexer.InterpolatedText("hello")]),
    lexer.TokEof,
  ]))
}

pub fn lex_interpolated_empty_test() {
  lexer.lex("``")
  |> should.equal(Ok([lexer.TokInterpolated([]), lexer.TokEof]))
}

pub fn lex_interpolated_with_expr_test() {
  lexer.lex("`hello {{name}}`")
  |> should.equal(Ok([
    lexer.TokInterpolated([
      lexer.InterpolatedText("hello "),
      lexer.InterpolatedExpr("name"),
    ]),
    lexer.TokEof,
  ]))
}

pub fn lex_interpolated_expr_only_test() {
  lexer.lex("`{{x}}`")
  |> should.equal(Ok([
    lexer.TokInterpolated([lexer.InterpolatedExpr("x")]),
    lexer.TokEof,
  ]))
}

pub fn lex_interpolated_multiple_exprs_test() {
  lexer.lex("`{{a}} and {{b}}`")
  |> should.equal(Ok([
    lexer.TokInterpolated([
      lexer.InterpolatedExpr("a"),
      lexer.InterpolatedText(" and "),
      lexer.InterpolatedExpr("b"),
    ]),
    lexer.TokEof,
  ]))
}

pub fn lex_interpolated_escape_brace_test() {
  lexer.lex("`a\\{b`")
  |> should.equal(Ok([
    lexer.TokInterpolated([lexer.InterpolatedText("a{b")]),
    lexer.TokEof,
  ]))
}

pub fn lex_interpolated_escape_backslash_test() {
  lexer.lex("`a\\\\b`")
  |> should.equal(Ok([
    lexer.TokInterpolated([lexer.InterpolatedText("a\\b")]),
    lexer.TokEof,
  ]))
}

// ---------------------------------------------------------------------------
// Labels and keywords
// ---------------------------------------------------------------------------

pub fn lex_label_test() {
  lexer.lex("foo")
  |> should.equal(Ok([lexer.TokLabel("foo"), lexer.TokEof]))
}

pub fn lex_label_with_digits_test() {
  lexer.lex("x1")
  |> should.equal(Ok([lexer.TokLabel("x1"), lexer.TokEof]))
}

pub fn lex_pub_test() {
  lexer.lex("pub")
  |> should.equal(Ok([lexer.TokPub, lexer.TokEof]))
}

pub fn lex_import_test() {
  lexer.lex("import")
  |> should.equal(Ok([lexer.TokImport, lexer.TokEof]))
}

pub fn lex_pub_prefix_not_reserved_test() {
  // `public` is not a reserved word
  lexer.lex("public")
  |> should.equal(Ok([lexer.TokLabel("public"), lexer.TokEof]))
}

pub fn lex_import_prefix_not_reserved_test() {
  lexer.lex("imported")
  |> should.equal(Ok([lexer.TokLabel("imported"), lexer.TokEof]))
}

// ---------------------------------------------------------------------------
// Discard / underscore
// ---------------------------------------------------------------------------

pub fn lex_discard_test() {
  lexer.lex("_")
  |> should.equal(Ok([lexer.TokUnderscore, lexer.TokEof]))
}

pub fn lex_label_underscore_prefix_test() {
  lexer.lex("_foo")
  |> should.equal(Ok([lexer.TokLabel("_foo"), lexer.TokEof]))
}

pub fn lex_discard_before_paren_test() {
  lexer.lex("_(")
  |> should.equal(Ok([lexer.TokUnderscore, lexer.TokLParen, lexer.TokEof]))
}

// ---------------------------------------------------------------------------
// Whitespace and comments
// ---------------------------------------------------------------------------

pub fn lex_whitespace_skipped_test() {
  lexer.lex("  foo  ")
  |> should.equal(Ok([lexer.TokLabel("foo"), lexer.TokEof]))
}

pub fn lex_newlines_skipped_test() {
  lexer.lex("\n\nfoo\n")
  |> should.equal(Ok([lexer.TokLabel("foo"), lexer.TokEof]))
}

pub fn lex_comment_test() {
  lexer.lex("# this is a comment\nfoo")
  |> should.equal(Ok([lexer.TokLabel("foo"), lexer.TokEof]))
}

pub fn lex_comment_at_end_test() {
  lexer.lex("foo # trailing comment")
  |> should.equal(Ok([lexer.TokLabel("foo"), lexer.TokEof]))
}

pub fn lex_comment_only_test() {
  lexer.lex("# just a comment")
  |> should.equal(Ok([lexer.TokEof]))
}

// ---------------------------------------------------------------------------
// Punctuation
// ---------------------------------------------------------------------------

pub fn lex_lparen_test() {
  lexer.lex("(")
  |> should.equal(Ok([lexer.TokLParen, lexer.TokEof]))
}

pub fn lex_rparen_test() {
  lexer.lex(")")
  |> should.equal(Ok([lexer.TokRParen, lexer.TokEof]))
}

pub fn lex_all_punctuation_test() {
  lexer.lex("( ) { } [ ] : , | . @ * '")
  |> should.equal(Ok([
    lexer.TokLParen,
    lexer.TokRParen,
    lexer.TokLBrace,
    lexer.TokRBrace,
    lexer.TokLBracket,
    lexer.TokRBracket,
    lexer.TokColon,
    lexer.TokComma,
    lexer.TokPipe,
    lexer.TokDot,
    lexer.TokAt,
    lexer.TokStar,
    lexer.TokTick,
    lexer.TokEof,
  ]))
}

// ---------------------------------------------------------------------------
// Composite expression examples
// ---------------------------------------------------------------------------

pub fn lex_pipe_expr_test() {
  lexer.lex("a | b")
  |> should.equal(Ok([
    lexer.TokLabel("a"),
    lexer.TokPipe,
    lexer.TokLabel("b"),
    lexer.TokEof,
  ]))
}

pub fn lex_identifier_type_test() {
  // `*Foo` — identifier-type annotation
  lexer.lex("*Foo")
  |> should.equal(Ok([lexer.TokStar, lexer.TokLabel("Foo"), lexer.TokEof]))
}

pub fn lex_alias_type_test() {
  // `'Foo` — alias-type annotation
  lexer.lex("'Foo")
  |> should.equal(Ok([lexer.TokTick, lexer.TokLabel("Foo"), lexer.TokEof]))
}

pub fn lex_definition_test() {
  // `x: 42`
  lexer.lex("x: 42")
  |> should.equal(Ok([
    lexer.TokLabel("x"),
    lexer.TokColon,
    lexer.TokInt("42"),
    lexer.TokEof,
  ]))
}

pub fn lex_macro_expr_test() {
  // `@log foo`
  lexer.lex("@log foo")
  |> should.equal(Ok([
    lexer.TokAt,
    lexer.TokLabel("log"),
    lexer.TokLabel("foo"),
    lexer.TokEof,
  ]))
}

pub fn lex_tuple_literal_test() {
  lexer.lex("{ x: 1, y: 2 }")
  |> should.equal(Ok([
    lexer.TokLBrace,
    lexer.TokLabel("x"),
    lexer.TokColon,
    lexer.TokInt("1"),
    lexer.TokComma,
    lexer.TokLabel("y"),
    lexer.TokColon,
    lexer.TokInt("2"),
    lexer.TokRBrace,
    lexer.TokEof,
  ]))
}

pub fn lex_array_literal_test() {
  lexer.lex("[1, 2, 3]")
  |> should.equal(Ok([
    lexer.TokLBracket,
    lexer.TokInt("1"),
    lexer.TokComma,
    lexer.TokInt("2"),
    lexer.TokComma,
    lexer.TokInt("3"),
    lexer.TokRBracket,
    lexer.TokEof,
  ]))
}

pub fn lex_sexpr_test() {
  // `(add 1 2)`
  lexer.lex("(add 1 2)")
  |> should.equal(Ok([
    lexer.TokLParen,
    lexer.TokLabel("add"),
    lexer.TokInt("1"),
    lexer.TokInt("2"),
    lexer.TokRParen,
    lexer.TokEof,
  ]))
}

pub fn lex_dot_access_test() {
  lexer.lex("obj.field")
  |> should.equal(Ok([
    lexer.TokLabel("obj"),
    lexer.TokDot,
    lexer.TokLabel("field"),
    lexer.TokEof,
  ]))
}

// ---------------------------------------------------------------------------
// Empty input
// ---------------------------------------------------------------------------

pub fn lex_empty_test() {
  lexer.lex("")
  |> should.equal(Ok([lexer.TokEof]))
}

// ---------------------------------------------------------------------------
// Error cases
// ---------------------------------------------------------------------------

pub fn lex_unterminated_string_test() {
  lexer.lex("\"hello")
  |> result.is_error
  |> should.be_true
}

pub fn lex_unterminated_interpolated_test() {
  lexer.lex("`hello")
  |> result.is_error
  |> should.be_true
}

pub fn lex_unterminated_interpolation_expr_test() {
  lexer.lex("`{{unclosed`")
  |> result.is_error
  |> should.be_true
}

pub fn lex_invalid_escape_test() {
  lexer.lex("\"\\z\"")
  |> result.is_error
  |> should.be_true
}

pub fn lex_unexpected_char_test() {
  lexer.lex("$")
  |> result.is_error
  |> should.be_true
}
