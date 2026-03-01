import gleam/list
import gleam/option.{type Option, None, Some}
import lexer.{
  type Token, TokAt, TokColon, TokComma, TokDot, TokEof, TokFloat, TokInt,
  TokInterpolated, TokLBrace, TokLBracket, TokLParen, TokPipe, TokRBrace,
  TokRBracket, TokRParen, TokStar, TokString, TokTick, TokUnderscore,
  TokLabel,
}

// ---------------------------------------------------------------------------
// AST types
// ---------------------------------------------------------------------------

pub type Program {
  Program(definitions: List(Definition))
}

pub type Definition {
  Definition(
    label: String,
    annotation: Option(TypeAnnotation),
    body: Expr,
  )
}

pub type TypeAnnotation {
  IdentifierType(TypeExpr)
  AliasType(TypeExpr)
}

pub type TypeExpr {
  TypeFunction(from: TypeExpr, to: TypeExpr)
  TypeTuple(entries: List(TypeTupleEntry))
  TypeArray(key: TypeExpr, value: TypeExpr)
  TypeLabel(String)
}

pub type TypeTupleEntry {
  TypeTupleEntry(label: String, type_expr: TypeExpr)
}

pub type Expr {
  MacroExpr(label: String, body: Expr)
  PipeExpr(head: PrefixExpr, clauses: List(PrefixExpr))
}

pub type PrefixExpr {
  PrefixExpr(head: AccessExpr, args: List(AccessExpr))
}

pub type AccessExpr {
  AccessExpr(primary: PrimaryExpr, accesses: List(Access))
}

pub type Access {
  DotAccess(label: String)
  BracketAccess(index: Expr)
}

pub type PrimaryExpr {
  FloatLit(String)
  IntLit(String)
  StringLit(String)
  InterpolatedLit(List(lexer.InterpolatedPart))
  Discard
  TypeLit(TypeAnnotation)
  ConditionalMapLit(List(ConditionalEntry))
  ArrayLit(List(ArrayEntry))
  TupleLit(List(TupleEntry))
  SExprLit(SExprBody)
  UnitLit
  NameRef(String)
}

pub type ConditionalEntry {
  ConditionalEntry(condition: Expr, value: Expr)
}

pub type ArrayEntry {
  ArrayIndexEntry(key: String, value: Expr)
  ArrayStringEntry(key: String, value: Expr)
  ArrayLabelEntry(key: String, value: Expr)
  ArrayValueEntry(value: Expr)
}

pub type TupleEntry {
  TupleIndexEntry(key: String, value: Expr)
  TupleLabelEntry(key: String, value: Expr)
  TupleValueEntry(value: Expr)
}

pub type SExprBody {
  LambdaBody(param: String, body: Expr)
  PipeBody(head: PrefixExpr, args: List(PrefixExpr), clauses: List(PrefixExpr))
  LeadingPipeBody(clauses: List(PrefixExpr))
}

// ---------------------------------------------------------------------------
// Parse errors
// ---------------------------------------------------------------------------

pub type ParseError {
  UnexpectedToken(got: Token, pos: Int)
  UnexpectedEof
}

// ---------------------------------------------------------------------------
// Parser state
// ---------------------------------------------------------------------------

type Tokens =
  List(#(Token, Int))

type ParseResult(a) =
  Result(#(a, Tokens), ParseError)

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

pub fn parse(tokens: List(Token)) -> Result(Program, ParseError) {
  let indexed = list.index_map(tokens, fn(t, i) { #(t, i) })
  case parse_program(indexed) {
    Ok(#(prog, _)) -> Ok(prog)
    Error(e) -> Error(e)
  }
}

// ---------------------------------------------------------------------------
// Program
// ---------------------------------------------------------------------------

fn parse_program(toks: Tokens) -> ParseResult(Program) {
  case parse_definitions(toks, []) {
    Ok(#(defs, rest)) ->
      case rest {
        [#(TokEof, _), ..] | [] -> Ok(#(Program(defs), []))
        [#(t, pos), ..] -> Error(UnexpectedToken(t, pos))
      }
    Error(e) -> Error(e)
  }
}

fn parse_definitions(
  toks: Tokens,
  acc: List(Definition),
) -> ParseResult(List(Definition)) {
  case toks {
    [] | [#(TokEof, _), ..] -> Ok(#(list.reverse(acc), toks))
    _ ->
      case parse_definition(toks) {
        Ok(#(def, rest)) -> parse_definitions(rest, [def, ..acc])
        Error(_) -> Ok(#(list.reverse(acc), toks))
      }
  }
}

fn parse_definition(toks: Tokens) -> ParseResult(Definition) {
  case toks {
    [#(TokLabel(name), _), ..rest1] -> {
      let #(annot, rest2) = try_parse_type_annotation(rest1)
      case rest2 {
        [#(TokColon, _), ..rest3] ->
          case parse_expr(rest3) {
            Ok(#(expr, rest4)) -> {
              let rest5 = skip_comma(rest4)
              Ok(#(Definition(name, annot, expr), rest5))
            }
            Error(e) -> Error(e)
          }
        [#(t, pos), ..] -> Error(UnexpectedToken(t, pos))
        [] -> Error(UnexpectedEof)
      }
    }
    [#(t, pos), ..] -> Error(UnexpectedToken(t, pos))
    [] -> Error(UnexpectedEof)
  }
}

fn skip_comma(toks: Tokens) -> Tokens {
  case toks {
    [#(TokComma, _), ..rest] -> rest
    _ -> toks
  }
}

// ---------------------------------------------------------------------------
// Type annotations
// ---------------------------------------------------------------------------

fn try_parse_type_annotation(toks: Tokens) -> #(Option(TypeAnnotation), Tokens) {
  case toks {
    [#(TokStar, _), ..rest] ->
      case parse_type_expr(rest) {
        Ok(#(te, rest2)) -> #(Some(IdentifierType(te)), rest2)
        Error(_) -> #(None, toks)
      }
    [#(TokTick, _), ..rest] ->
      case parse_type_expr(rest) {
        Ok(#(te, rest2)) -> #(Some(AliasType(te)), rest2)
        Error(_) -> #(None, toks)
      }
    _ -> #(None, toks)
  }
}

fn parse_type_expr(toks: Tokens) -> ParseResult(TypeExpr) {
  case toks {
    // TypeFunction: '(' TypeExpr '|' TypeExpr ')'
    [#(TokLParen, _), ..rest] ->
      case parse_type_expr(rest) {
        Ok(#(from, [#(TokPipe, _), ..rest2])) ->
          case parse_type_expr(rest2) {
            Ok(#(to, [#(TokRParen, _), ..rest3])) ->
              Ok(#(TypeFunction(from, to), rest3))
            Ok(#(_, [#(t, pos), ..])) -> Error(UnexpectedToken(t, pos))
            Ok(#(_, [])) -> Error(UnexpectedEof)
            Error(e) -> Error(e)
          }
        Ok(#(_, [#(t, pos), ..])) -> Error(UnexpectedToken(t, pos))
        Ok(#(_, [])) -> Error(UnexpectedEof)
        Error(e) -> Error(e)
      }
    // TypeTuple: '{' TypeTupleEntry (',' TypeTupleEntry)* '}'
    [#(TokLBrace, _), ..rest] ->
      case parse_type_tuple_entries(rest, []) {
        Ok(#(entries, [#(TokRBrace, _), ..rest2])) ->
          Ok(#(TypeTuple(entries), rest2))
        Ok(#(_, [#(t, pos), ..])) -> Error(UnexpectedToken(t, pos))
        Ok(#(_, [])) -> Error(UnexpectedEof)
        Error(e) -> Error(e)
      }
    // TypeArray: '[' TypeExpr ':' TypeExpr ']'
    [#(TokLBracket, _), ..rest] ->
      case parse_type_expr(rest) {
        Ok(#(key, [#(TokColon, _), ..rest2])) ->
          case parse_type_expr(rest2) {
            Ok(#(val, [#(TokRBracket, _), ..rest3])) ->
              Ok(#(TypeArray(key, val), rest3))
            Ok(#(_, [#(t, pos), ..])) -> Error(UnexpectedToken(t, pos))
            Ok(#(_, [])) -> Error(UnexpectedEof)
            Error(e) -> Error(e)
          }
        Ok(#(_, [#(t, pos), ..])) -> Error(UnexpectedToken(t, pos))
        Ok(#(_, [])) -> Error(UnexpectedEof)
        Error(e) -> Error(e)
      }
    // Label
    [#(TokLabel(name), _), ..rest] -> Ok(#(TypeLabel(name), rest))
    [#(t, pos), ..] -> Error(UnexpectedToken(t, pos))
    [] -> Error(UnexpectedEof)
  }
}

fn parse_type_tuple_entries(
  toks: Tokens,
  acc: List(TypeTupleEntry),
) -> ParseResult(List(TypeTupleEntry)) {
  case parse_type_tuple_entry(toks) {
    Ok(#(entry, rest)) -> {
      let acc2 = [entry, ..acc]
      case rest {
        [#(TokComma, _), ..rest2] -> parse_type_tuple_entries(rest2, acc2)
        _ -> Ok(#(list.reverse(acc2), rest))
      }
    }
    Error(_) -> Ok(#(list.reverse(acc), toks))
  }
}

fn parse_type_tuple_entry(toks: Tokens) -> ParseResult(TypeTupleEntry) {
  case toks {
    [#(TokLabel(name), _), #(TokColon, _), ..rest] ->
      case parse_type_expr(rest) {
        Ok(#(te, rest2)) -> Ok(#(TypeTupleEntry(name, te), rest2))
        Error(e) -> Error(e)
      }
    [#(t, pos), ..] -> Error(UnexpectedToken(t, pos))
    [] -> Error(UnexpectedEof)
  }
}

// ---------------------------------------------------------------------------
// Expressions
// ---------------------------------------------------------------------------

fn parse_expr(toks: Tokens) -> ParseResult(Expr) {
  case toks {
    // MacroExpr: '@' Label Expr
    [#(TokAt, _), #(TokLabel(name), _), ..rest] ->
      case parse_expr(rest) {
        Ok(#(body, rest2)) -> Ok(#(MacroExpr(name, body), rest2))
        Error(e) -> Error(e)
      }
    _ -> parse_pipe_expr(toks)
  }
}

fn parse_pipe_expr(toks: Tokens) -> ParseResult(Expr) {
  case parse_prefix_expr(toks) {
    Ok(#(head, rest)) ->
      case parse_pipe_clauses(rest, []) {
        Ok(#(clauses, rest2)) -> Ok(#(PipeExpr(head, clauses), rest2))
        Error(e) -> Error(e)
      }
    Error(e) -> Error(e)
  }
}

fn parse_pipe_clauses(
  toks: Tokens,
  acc: List(PrefixExpr),
) -> ParseResult(List(PrefixExpr)) {
  case toks {
    [#(TokPipe, _), ..rest] ->
      case parse_prefix_expr(rest) {
        Ok(#(clause, rest2)) -> parse_pipe_clauses(rest2, [clause, ..acc])
        Error(e) -> Error(e)
      }
    _ -> Ok(#(list.reverse(acc), toks))
  }
}

fn parse_prefix_expr(toks: Tokens) -> ParseResult(PrefixExpr) {
  case parse_access_expr(toks) {
    Ok(#(head, rest)) ->
      case parse_access_exprs(rest, []) {
        Ok(#(args, rest2)) -> Ok(#(PrefixExpr(head, args), rest2))
        Error(e) -> Error(e)
      }
    Error(e) -> Error(e)
  }
}

// Tokens that can never start a primary expression — used to break greedy loops
fn is_stopper(t: Token) -> Bool {
  case t {
    TokEof | TokRParen | TokRBrace | TokRBracket | TokPipe | TokComma
    | TokColon -> True
    _ -> False
  }
}

fn parse_access_exprs(
  toks: Tokens,
  acc: List(AccessExpr),
) -> ParseResult(List(AccessExpr)) {
  case toks {
    [] -> Ok(#(list.reverse(acc), toks))
    [#(t, _), ..] if is_stopper(t) -> Ok(#(list.reverse(acc), toks))
    _ ->
      case parse_access_expr(toks) {
        Ok(#(ae, rest)) -> parse_access_exprs(rest, [ae, ..acc])
        Error(_) -> Ok(#(list.reverse(acc), toks))
      }
  }
}

fn parse_access_expr(toks: Tokens) -> ParseResult(AccessExpr) {
  case parse_primary_expr(toks) {
    Ok(#(primary, rest)) ->
      case parse_accesses(rest, []) {
        Ok(#(accesses, rest2)) -> Ok(#(AccessExpr(primary, accesses), rest2))
        Error(e) -> Error(e)
      }
    Error(e) -> Error(e)
  }
}

fn parse_accesses(
  toks: Tokens,
  acc: List(Access),
) -> ParseResult(List(Access)) {
  case toks {
    // DotAccess: '.' Label (not '[')
    [#(TokDot, _), #(TokLabel(name), _), ..rest] ->
      parse_accesses(rest, [DotAccess(name), ..acc])
    // BracketAccess: '.' '[' Expr ']'  — backtrack if ']' not found
    [#(TokDot, _), #(TokLBracket, _), ..rest] ->
      case parse_expr(rest) {
        Ok(#(expr, [#(TokRBracket, _), ..rest2])) ->
          parse_accesses(rest2, [BracketAccess(expr), ..acc])
        _ -> Ok(#(list.reverse(acc), toks))
      }
    // BracketAccess: '[' Expr ']'  — backtrack if ']' not found
    [#(TokLBracket, _), ..rest] ->
      case parse_expr(rest) {
        Ok(#(expr, [#(TokRBracket, _), ..rest2])) ->
          parse_accesses(rest2, [BracketAccess(expr), ..acc])
        _ -> Ok(#(list.reverse(acc), toks))
      }
    _ -> Ok(#(list.reverse(acc), toks))
  }
}

// ---------------------------------------------------------------------------
// Primary expressions
// ---------------------------------------------------------------------------

fn parse_primary_expr(toks: Tokens) -> ParseResult(PrimaryExpr) {
  case toks {
    [#(TokFloat(v), _), ..rest] -> Ok(#(FloatLit(v), rest))
    [#(TokInt(v), _), ..rest] -> Ok(#(IntLit(v), rest))
    [#(TokString(v), _), ..rest] -> Ok(#(StringLit(v), rest))
    [#(TokInterpolated(parts), _), ..rest] -> Ok(#(InterpolatedLit(parts), rest))
    [#(TokUnderscore, _), ..rest] -> Ok(#(Discard, rest))

    // TypeLiteral: '*' TypeExpr or '\'' TypeExpr
    [#(TokStar, _), ..rest] ->
      case parse_type_expr(rest) {
        Ok(#(te, rest2)) -> Ok(#(TypeLit(IdentifierType(te)), rest2))
        Error(e) -> Error(e)
      }
    [#(TokTick, _), ..rest] ->
      case parse_type_expr(rest) {
        Ok(#(te, rest2)) -> Ok(#(TypeLit(AliasType(te)), rest2))
        Error(e) -> Error(e)
      }

    // UnitLiteral: '(' ')' — must check before SExpr
    [#(TokLParen, _), #(TokRParen, _), ..rest] -> Ok(#(UnitLit, rest))

    // SExpr: '(' SExprBody ')'
    [#(TokLParen, _), ..rest] ->
      case parse_sexpr_body(rest) {
        Ok(#(body, [#(TokRParen, _), ..rest2])) -> Ok(#(SExprLit(body), rest2))
        Ok(#(_, [#(t, pos), ..])) -> Error(UnexpectedToken(t, pos))
        Ok(#(_, [])) -> Error(UnexpectedEof)
        Error(e) -> Error(e)
      }

    // '[' — ConditionalMap or ArrayLiteral
    [#(TokLBracket, _), ..rest] -> parse_bracket_literal(rest)

    // TupleLiteral: '{' ... '}'
    [#(TokLBrace, _), ..rest] ->
      case parse_tuple_entries(rest, []) {
        Ok(#(entries, [#(TokRBrace, _), ..rest2])) ->
          Ok(#(TupleLit(entries), rest2))
        Ok(#(_, [#(t, pos), ..])) -> Error(UnexpectedToken(t, pos))
        Ok(#(_, [])) -> Error(UnexpectedEof)
        Error(e) -> Error(e)
      }

    // NameRef
    [#(TokLabel(name), _), ..rest] -> Ok(#(NameRef(name), rest))

    [#(t, pos), ..] -> Error(UnexpectedToken(t, pos))
    [] -> Error(UnexpectedEof)
  }
}

// Decide between ConditionalMap and ArrayLiteral after consuming '['
fn parse_bracket_literal(toks: Tokens) -> ParseResult(PrimaryExpr) {
  case toks {
    // Empty array
    [#(TokRBracket, _), ..rest] -> Ok(#(ArrayLit([]), rest))
    // May be ConditionalMap if it starts with '(' — try it, fall back to array
    [#(TokLParen, _), ..] ->
      case parse_conditional_entries(toks, []) {
        Ok(#(entries, [#(TokRBracket, _), ..rest])) ->
          Ok(#(ConditionalMapLit(entries), rest))
        // Conditional parse failed or no ']' — fall back to array
        _ ->
          case parse_array_entries(toks, []) {
            Ok(#(entries, [#(TokRBracket, _), ..rest])) ->
              Ok(#(ArrayLit(entries), rest))
            Ok(#(_, [#(t, pos), ..])) -> Error(UnexpectedToken(t, pos))
            Ok(#(_, [])) -> Error(UnexpectedEof)
            Error(e) -> Error(e)
          }
      }
    _ ->
      case parse_array_entries(toks, []) {
        Ok(#(entries, [#(TokRBracket, _), ..rest])) ->
          Ok(#(ArrayLit(entries), rest))
        Ok(#(_, [#(t, pos), ..])) -> Error(UnexpectedToken(t, pos))
        Ok(#(_, [])) -> Error(UnexpectedEof)
        Error(e) -> Error(e)
      }
  }
}

// ---------------------------------------------------------------------------
// Conditional map entries
// ---------------------------------------------------------------------------

fn parse_conditional_entries(
  toks: Tokens,
  acc: List(ConditionalEntry),
) -> ParseResult(List(ConditionalEntry)) {
  case toks {
    [#(TokLParen, _), ..rest] ->
      case parse_expr(rest) {
        Ok(#(cond, [#(TokRParen, _), #(TokColon, _), ..rest2])) ->
          case parse_expr(rest2) {
            Ok(#(value, rest3)) -> {
              let acc2 = [ConditionalEntry(cond, value), ..acc]
              case rest3 {
                [#(TokComma, _), ..rest4] ->
                  parse_conditional_entries(rest4, acc2)
                _ -> Ok(#(list.reverse(acc2), rest3))
              }
            }
            Error(e) -> Error(e)
          }
        Ok(#(_, [#(t, pos), ..])) -> Error(UnexpectedToken(t, pos))
        Ok(#(_, [])) -> Error(UnexpectedEof)
        Error(e) -> Error(e)
      }
    _ -> Ok(#(list.reverse(acc), toks))
  }
}

// ---------------------------------------------------------------------------
// Array entries
// ---------------------------------------------------------------------------

fn parse_array_entries(
  toks: Tokens,
  acc: List(ArrayEntry),
) -> ParseResult(List(ArrayEntry)) {
  case parse_array_entry(toks) {
    Ok(#(entry, rest)) -> {
      let acc2 = [entry, ..acc]
      case rest {
        [#(TokComma, _), ..rest2] -> parse_array_entries(rest2, acc2)
        _ -> Ok(#(list.reverse(acc2), rest))
      }
    }
    Error(_) -> Ok(#(list.reverse(acc), toks))
  }
}

fn parse_array_entry(toks: Tokens) -> ParseResult(ArrayEntry) {
  case toks {
    // StringKey ':' Expr
    [#(TokString(key), _), #(TokColon, _), ..rest] ->
      case parse_expr(rest) {
        Ok(#(v, rest2)) -> Ok(#(ArrayStringEntry(key, v), rest2))
        Error(e) -> Error(e)
      }
    // IntegerKey ':' Expr
    [#(TokInt(key), _), #(TokColon, _), ..rest] ->
      case parse_expr(rest) {
        Ok(#(v, rest2)) -> Ok(#(ArrayIndexEntry(key, v), rest2))
        Error(e) -> Error(e)
      }
    // Label ':' Expr
    [#(TokLabel(key), _), #(TokColon, _), ..rest] ->
      case parse_expr(rest) {
        Ok(#(v, rest2)) -> Ok(#(ArrayLabelEntry(key, v), rest2))
        Error(e) -> Error(e)
      }
    // Just Expr
    _ ->
      case parse_expr(toks) {
        Ok(#(v, rest)) -> Ok(#(ArrayValueEntry(v), rest))
        Error(e) -> Error(e)
      }
  }
}

// ---------------------------------------------------------------------------
// Tuple entries
// ---------------------------------------------------------------------------

fn parse_tuple_entries(
  toks: Tokens,
  acc: List(TupleEntry),
) -> ParseResult(List(TupleEntry)) {
  case toks {
    [#(TokRBrace, _), ..] -> Ok(#(list.reverse(acc), toks))
    _ ->
      case parse_tuple_entry(toks) {
        Ok(#(entry, rest)) -> {
          let acc2 = [entry, ..acc]
          case rest {
            [#(TokComma, _), ..rest2] -> parse_tuple_entries(rest2, acc2)
            _ -> Ok(#(list.reverse(acc2), rest))
          }
        }
        Error(_) -> Ok(#(list.reverse(acc), toks))
      }
  }
}

fn parse_tuple_entry(toks: Tokens) -> ParseResult(TupleEntry) {
  case toks {
    // IntegerKey ':' Expr
    [#(TokInt(key), _), #(TokColon, _), ..rest] ->
      case parse_expr(rest) {
        Ok(#(v, rest2)) -> Ok(#(TupleIndexEntry(key, v), rest2))
        Error(e) -> Error(e)
      }
    // Label ':' Expr
    [#(TokLabel(key), _), #(TokColon, _), ..rest] ->
      case parse_expr(rest) {
        Ok(#(v, rest2)) -> Ok(#(TupleLabelEntry(key, v), rest2))
        Error(e) -> Error(e)
      }
    // Just Expr
    _ ->
      case parse_expr(toks) {
        Ok(#(v, rest)) -> Ok(#(TupleValueEntry(v), rest))
        Error(e) -> Error(e)
      }
  }
}

// ---------------------------------------------------------------------------
// S-expression body
// ---------------------------------------------------------------------------

fn parse_sexpr_body(toks: Tokens) -> ParseResult(SExprBody) {
  case toks {
    // LambdaBody: Label ':' Expr
    [#(TokLabel(param), _), #(TokColon, _), ..rest] ->
      case parse_expr(rest) {
        Ok(#(body, rest2)) -> Ok(#(LambdaBody(param, body), rest2))
        Error(e) -> Error(e)
      }
    // LeadingPipeBody: ('|' PipeClauseInner)+
    [#(TokPipe, _), ..] ->
      case parse_pipe_clauses_inner(toks, []) {
        Ok(#(clauses, rest)) -> Ok(#(LeadingPipeBody(clauses), rest))
        Error(e) -> Error(e)
      }
    // PipeBody: PrefixExpr (PrefixExpr)* ('|' PipeClauseInner)*
    _ ->
      case parse_prefix_expr(toks) {
        Ok(#(head, rest)) ->
          case parse_sexpr_args(rest, []) {
            Ok(#(args, rest2)) ->
              case parse_pipe_clauses_inner(rest2, []) {
                Ok(#(clauses, rest3)) ->
                  Ok(#(PipeBody(head, args, clauses), rest3))
                Error(e) -> Error(e)
              }
            Error(e) -> Error(e)
          }
        Error(e) -> Error(e)
      }
  }
}

// Parse additional PrefixExprs (space-separated) in a PipeBody until '|' or ')'
fn parse_sexpr_args(
  toks: Tokens,
  acc: List(PrefixExpr),
) -> ParseResult(List(PrefixExpr)) {
  case toks {
    [#(TokPipe, _), ..] | [#(TokRParen, _), ..] | [] ->
      Ok(#(list.reverse(acc), toks))
    _ ->
      case parse_prefix_expr(toks) {
        Ok(#(arg, rest)) -> parse_sexpr_args(rest, [arg, ..acc])
        Error(_) -> Ok(#(list.reverse(acc), toks))
      }
  }
}

// Parse ('|' PrefixExpr)+ clauses inside an S-expr
fn parse_pipe_clauses_inner(
  toks: Tokens,
  acc: List(PrefixExpr),
) -> ParseResult(List(PrefixExpr)) {
  case toks {
    [#(TokPipe, _), ..rest] ->
      case parse_prefix_expr(rest) {
        Ok(#(clause, rest2)) ->
          parse_pipe_clauses_inner(rest2, [clause, ..acc])
        Error(e) -> Error(e)
      }
    _ -> Ok(#(list.reverse(acc), toks))
  }
}
