import gleam/list
import gleam/result
import gleam/string

// ---------------------------------------------------------------------------
// Token types
// ---------------------------------------------------------------------------

pub type Token {
  /// A floating-point literal, e.g. `3.14` or `-2.0`
  TokFloat(String)
  /// An integer literal, e.g. `42` or `-7`
  TokInt(String)
  /// A string literal — the unescaped content without surrounding quotes
  TokString(String)
  /// A backtick-delimited interpolated string
  TokInterpolated(List(InterpolatedPart))
  /// An identifier that is not a reserved word
  TokLabel(String)
  /// The `pub` keyword
  TokPub
  /// The `import` keyword
  TokImport
  /// `(`
  TokLParen
  /// `)`
  TokRParen
  /// `{`
  TokLBrace
  /// `}`
  TokRBrace
  /// `[`
  TokLBracket
  /// `]`
  TokRBracket
  /// `:`
  TokColon
  /// `,`
  TokComma
  /// `|`
  TokPipe
  /// `.`
  TokDot
  /// `@`
  TokAt
  /// `*` — used for identifier-type annotations
  TokStar
  /// `'` — used for alias-type annotations
  TokTick
  /// `_` — the discard pattern
  TokUnderscore
  /// Sentinel placed at the end of every successful token stream
  TokEof
}

// ---------------------------------------------------------------------------
// Interpolated-string parts
// ---------------------------------------------------------------------------

pub type InterpolatedPart {
  /// A run of literal text inside a backtick string
  InterpolatedText(String)
  /// Raw source of the expression between `{{` … `}}`
  InterpolatedExpr(String)
}

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

pub type LexError {
  UnexpectedChar(String, Int)
  UnterminatedString(Int)
  UnterminatedInterpolated(Int)
  InvalidEscapeSequence(String, Int)
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Turn *source* into a flat list of tokens ending with `TokEof`.
pub fn lex(source: String) -> Result(List(Token), LexError) {
  let chars = string.to_graphemes(source)
  do_lex(chars, 0, [])
  |> result.map(list.reverse)
}

// ---------------------------------------------------------------------------
// Main dispatch loop
// ---------------------------------------------------------------------------

fn do_lex(
  chars: List(String),
  pos: Int,
  acc: List(Token),
) -> Result(List(Token), LexError) {
  case chars {
    // End of input
    [] -> Ok([TokEof, ..acc])

    // Whitespace — skip silently
    [c, ..rest]
      if c == " " || c == "\t" || c == "\r" || c == "\n"
    -> do_lex(rest, pos + 1, acc)

    // Line comments  `# …`
    ["#", ..rest] -> do_lex(skip_comment(rest), pos, acc)

    // Negative number  `-5`  or  `-3.14`
    ["-", c, ..rest] -> {
      case is_digit(c) {
        True -> lex_number("-", [c, ..rest], pos, acc)
        False -> Error(UnexpectedChar("-", pos))
      }
    }

    // String literal  `"…"`
    ["\"", ..rest] -> lex_string(rest, pos + 1, acc)

    // Interpolated string  `` `…` ``
    ["`", ..rest] -> lex_interpolated(rest, pos + 1, acc)

    // Underscore: Discard `_` vs. label starting with `_foo`
    ["_", c, ..rest] -> {
      case is_alnum(c) {
        True -> lex_label(["_", c, ..rest], pos, acc)
        False -> do_lex([c, ..rest], pos + 1, [TokUnderscore, ..acc])
      }
    }
    ["_"] -> do_lex([], pos + 1, [TokUnderscore, ..acc])

    // Single-character punctuation
    ["(", ..rest] -> do_lex(rest, pos + 1, [TokLParen, ..acc])
    [")", ..rest] -> do_lex(rest, pos + 1, [TokRParen, ..acc])
    ["{", ..rest] -> do_lex(rest, pos + 1, [TokLBrace, ..acc])
    ["}", ..rest] -> do_lex(rest, pos + 1, [TokRBrace, ..acc])
    ["[", ..rest] -> do_lex(rest, pos + 1, [TokLBracket, ..acc])
    ["]", ..rest] -> do_lex(rest, pos + 1, [TokRBracket, ..acc])
    [":", ..rest] -> do_lex(rest, pos + 1, [TokColon, ..acc])
    [",", ..rest] -> do_lex(rest, pos + 1, [TokComma, ..acc])
    ["|", ..rest] -> do_lex(rest, pos + 1, [TokPipe, ..acc])
    [".", ..rest] -> do_lex(rest, pos + 1, [TokDot, ..acc])
    ["@", ..rest] -> do_lex(rest, pos + 1, [TokAt, ..acc])
    ["*", ..rest] -> do_lex(rest, pos + 1, [TokStar, ..acc])
    ["'", ..rest] -> do_lex(rest, pos + 1, [TokTick, ..acc])

    // Positive number, label/keyword, or unknown character
    [c, ..] -> {
      case is_digit(c) {
        True -> lex_number("", chars, pos, acc)
        False ->
          case is_alpha_start(c) {
            True -> lex_label(chars, pos, acc)
            False -> Error(UnexpectedChar(c, pos))
          }
      }
    }
  }
}

// ---------------------------------------------------------------------------
// Number lexing
// ---------------------------------------------------------------------------

fn lex_number(
  sign: String,
  chars: List(String),
  pos: Int,
  acc: List(Token),
) -> Result(List(Token), LexError) {
  let #(int_part, rest) = collect_while(chars, is_digit, "")
  case rest {
    // Float: digits '.' digits
    [".", d, ..tail] -> {
      case is_digit(d) {
        True -> {
          let #(frac_part, rest2) = collect_while([d, ..tail], is_digit, "")
          let value = sign <> int_part <> "." <> frac_part
          do_lex(rest2, pos + string.length(value), [TokFloat(value), ..acc])
        }
        // '.' not followed by a digit — emit integer, leave '.' for next pass
        False -> {
          let value = sign <> int_part
          do_lex(rest, pos + string.length(value), [TokInt(value), ..acc])
        }
      }
    }
    // Integer (no decimal point)
    _ -> {
      let value = sign <> int_part
      do_lex(rest, pos + string.length(value), [TokInt(value), ..acc])
    }
  }
}

// ---------------------------------------------------------------------------
// String-literal lexing
// ---------------------------------------------------------------------------

fn lex_string(
  chars: List(String),
  pos: Int,
  acc: List(Token),
) -> Result(List(Token), LexError) {
  case collect_string_chars(chars, pos, "") {
    Ok(#(content, rest)) ->
      do_lex(rest, pos + string.length(content) + 1, [
        TokString(content),
        ..acc
      ])
    Error(e) -> Error(e)
  }
}

fn collect_string_chars(
  chars: List(String),
  pos: Int,
  acc: String,
) -> Result(#(String, List(String)), LexError) {
  case chars {
    [] -> Error(UnterminatedString(pos))
    ["\"", ..rest] -> Ok(#(acc, rest))
    ["\\", c, ..rest] ->
      case c {
        "\"" -> collect_string_chars(rest, pos + 2, acc <> "\"")
        "\\" -> collect_string_chars(rest, pos + 2, acc <> "\\")
        "/" -> collect_string_chars(rest, pos + 2, acc <> "/")
        "n" -> collect_string_chars(rest, pos + 2, acc <> "\n")
        "r" -> collect_string_chars(rest, pos + 2, acc <> "\r")
        "t" -> collect_string_chars(rest, pos + 2, acc <> "\t")
        "b" -> collect_string_chars(rest, pos + 2, acc <> "\u{0008}")
        "f" -> collect_string_chars(rest, pos + 2, acc <> "\u{000C}")
        _ -> Error(InvalidEscapeSequence(c, pos + 1))
      }
    [c, ..rest] -> collect_string_chars(rest, pos + 1, acc <> c)
  }
}

// ---------------------------------------------------------------------------
// Interpolated-string lexing
// ---------------------------------------------------------------------------

fn lex_interpolated(
  chars: List(String),
  pos: Int,
  acc: List(Token),
) -> Result(List(Token), LexError) {
  case collect_interpolated_parts(chars, pos, "", []) {
    Ok(#(parts, rest)) ->
      do_lex(rest, pos, [TokInterpolated(parts), ..acc])
    Error(e) -> Error(e)
  }
}

fn collect_interpolated_parts(
  chars: List(String),
  pos: Int,
  text_acc: String,
  parts: List(InterpolatedPart),
) -> Result(#(List(InterpolatedPart), List(String)), LexError) {
  case chars {
    [] -> Error(UnterminatedInterpolated(pos))

    // End of interpolated string
    ["`", ..rest] -> {
      let final_parts = case text_acc {
        "" -> list.reverse(parts)
        t -> list.reverse([InterpolatedText(t), ..parts])
      }
      Ok(#(final_parts, rest))
    }

    // Start of an interpolated expression  `{{ … }}`
    ["{", "{", ..rest] -> {
      let parts2 = case text_acc {
        "" -> parts
        t -> [InterpolatedText(t), ..parts]
      }
      case collect_interpolation_expr(rest, pos + 2, 0, "") {
        Ok(#(expr, rest2)) ->
          collect_interpolated_parts(rest2, pos, "", [
            InterpolatedExpr(expr),
            ..parts2
          ])
        Error(e) -> Error(e)
      }
    }

    // Escape sequences inside interpolated strings
    ["\\", "{", ..rest] ->
      collect_interpolated_parts(rest, pos + 2, text_acc <> "{", parts)
    ["\\", "\\", ..rest] ->
      collect_interpolated_parts(rest, pos + 2, text_acc <> "\\", parts)

    // Ordinary character
    [c, ..rest] ->
      collect_interpolated_parts(rest, pos + 1, text_acc <> c, parts)
  }
}

/// Collect the raw expression source between `{{` and `}}`.
/// Tracks `{{ }}` nesting depth so that expressions containing
/// nested interpolations are handled correctly.
fn collect_interpolation_expr(
  chars: List(String),
  pos: Int,
  depth: Int,
  acc: String,
) -> Result(#(String, List(String)), LexError) {
  case chars {
    [] -> Error(UnterminatedInterpolated(pos))
    ["}", "}", ..rest] if depth == 0 -> Ok(#(string.trim(acc), rest))
    ["}", "}", ..rest] ->
      collect_interpolation_expr(rest, pos + 2, depth - 1, acc <> "}}")
    ["{", "{", ..rest] ->
      collect_interpolation_expr(rest, pos + 2, depth + 1, acc <> "{{")
    [c, ..rest] ->
      collect_interpolation_expr(rest, pos + 1, depth, acc <> c)
  }
}

// ---------------------------------------------------------------------------
// Label / keyword lexing
// ---------------------------------------------------------------------------

fn lex_label(
  chars: List(String),
  pos: Int,
  acc: List(Token),
) -> Result(List(Token), LexError) {
  let #(name, rest) = collect_while(chars, is_alnum, "")
  let tok = case name {
    "pub" -> TokPub
    "import" -> TokImport
    _ -> TokLabel(name)
  }
  do_lex(rest, pos + string.length(name), [tok, ..acc])
}

// ---------------------------------------------------------------------------
// Comment skipping
// ---------------------------------------------------------------------------

fn skip_comment(chars: List(String)) -> List(String) {
  case chars {
    [] -> []
    ["\r", "\n", ..rest] -> rest
    ["\r", ..rest] -> rest
    ["\n", ..rest] -> rest
    [_, ..rest] -> skip_comment(rest)
  }
}

// ---------------------------------------------------------------------------
// Utility: collect characters while predicate holds
// ---------------------------------------------------------------------------

fn collect_while(
  chars: List(String),
  pred: fn(String) -> Bool,
  acc: String,
) -> #(String, List(String)) {
  case chars {
    [] -> #(acc, [])
    [c, ..rest] ->
      case pred(c) {
        True -> collect_while(rest, pred, acc <> c)
        False -> #(acc, chars)
      }
  }
}

// ---------------------------------------------------------------------------
// Character-class helpers
// ---------------------------------------------------------------------------

fn codepoint(c: String) -> Int {
  case string.to_utf_codepoints(c) {
    [cp, ..] -> string.utf_codepoint_to_int(cp)
    [] -> -1
  }
}

fn is_digit(c: String) -> Bool {
  let n = codepoint(c)
  n >= 48 && n <= 57
}

fn is_alpha_start(c: String) -> Bool {
  let n = codepoint(c)
  n == 95 || n >= 65 && n <= 90 || n >= 97 && n <= 122
}

fn is_alnum(c: String) -> Bool {
  let n = codepoint(c)
  n == 95
  || n >= 65 && n <= 90
  || n >= 97 && n <= 122
  || n >= 48 && n <= 57
}
