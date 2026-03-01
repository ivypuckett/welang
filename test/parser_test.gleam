import gleam/list
import gleam/option.{None, Some}
import gleeunit/should
import lexer
import parser

// Helper: lex then parse
fn parse_src(src: String) -> Result(parser.Program, parser.ParseError) {
  case lexer.lex(src) {
    Ok(tokens) -> parser.parse(tokens)
    Error(_) -> Error(parser.UnexpectedEof)
  }
}

// Helper: extract the body of a single-definition program
fn single_body(src: String) -> parser.Expr {
  let assert Ok(parser.Program([parser.Definition(_, _, body)])) =
    parse_src(src)
  body
}

// Helper: get the head primary of a simple pipe expression
fn simple_primary(expr: parser.Expr) -> parser.PrimaryExpr {
  let assert parser.PipeExpr(
    parser.PrefixExpr(parser.AccessExpr(primary, _), _),
    _,
  ) = expr
  primary
}

// ---------------------------------------------------------------------------
// Integer literals
// ---------------------------------------------------------------------------

pub fn parse_integer_test() {
  single_body("z: 42")
  |> simple_primary
  |> should.equal(parser.IntLit("42"))
}

pub fn parse_negative_integer_test() {
  single_body("z: -5")
  |> simple_primary
  |> should.equal(parser.IntLit("-5"))
}

// ---------------------------------------------------------------------------
// Float literals
// ---------------------------------------------------------------------------

pub fn parse_float_test() {
  single_body("z: 3.14")
  |> simple_primary
  |> should.equal(parser.FloatLit("3.14"))
}

// ---------------------------------------------------------------------------
// String literals
// ---------------------------------------------------------------------------

pub fn parse_string_test() {
  single_body("z: \"hello\"")
  |> simple_primary
  |> should.equal(parser.StringLit("hello"))
}

// ---------------------------------------------------------------------------
// Unit literal
// ---------------------------------------------------------------------------

pub fn parse_unit_test() {
  single_body("z: ()")
  |> simple_primary
  |> should.equal(parser.UnitLit)
}

// ---------------------------------------------------------------------------
// Discard
// ---------------------------------------------------------------------------

pub fn parse_discard_test() {
  single_body("z: _")
  |> simple_primary
  |> should.equal(parser.Discard)
}

// ---------------------------------------------------------------------------
// NameRef
// ---------------------------------------------------------------------------

pub fn parse_nameref_test() {
  single_body("z: foo")
  |> simple_primary
  |> should.equal(parser.NameRef("foo"))
}

// ---------------------------------------------------------------------------
// Pipe expression
// ---------------------------------------------------------------------------

// A bare top-level pipe in a definition is invalid — must be wrapped in (...)
pub fn parse_pipe_expr_error_test() {
  parse_src("z: a | b")
  |> should.be_error()
}

// Pipe wrapped in an S-expression is the correct monadic form
pub fn parse_pipe_expr_test() {
  let body = single_body("z: (a | b)")
  let assert parser.PipeExpr(
    parser.PrefixExpr(
      parser.AccessExpr(
        parser.SExprLit(parser.PipeBody(
          parser.PrefixExpr(parser.AccessExpr(parser.NameRef("a"), []), []),
          [],
          [parser.PrefixExpr(parser.AccessExpr(parser.NameRef("b"), []), [])],
        )),
        [],
      ),
      [],
    ),
    [],
  ) = body
  should.be_true(True)
}

// ---------------------------------------------------------------------------
// Macro expression
// ---------------------------------------------------------------------------

pub fn parse_macro_test() {
  let body = single_body("z: @log foo")
  let assert parser.MacroExpr("log", inner) = body
  inner
  |> simple_primary
  |> should.equal(parser.NameRef("foo"))
}

// ---------------------------------------------------------------------------
// Dot access
// ---------------------------------------------------------------------------

pub fn parse_dot_access_test() {
  let body = single_body("z: obj.field")
  let assert parser.PipeExpr(
    parser.PrefixExpr(
      parser.AccessExpr(parser.NameRef("obj"), [parser.DotAccess("field")]),
      _,
    ),
    _,
  ) = body
  should.be_true(True)
}

// ---------------------------------------------------------------------------
// S-expression: function application
// ---------------------------------------------------------------------------

pub fn parse_sexpr_application_test() {
  let body = single_body("z: (print foo)")
  let assert parser.PipeExpr(
    parser.PrefixExpr(
      parser.AccessExpr(
        parser.SExprLit(parser.PipeBody(
          parser.PrefixExpr(parser.AccessExpr(parser.NameRef("print"), []), _),
          _,
          _,
        )),
        _,
      ),
      _,
    ),
    _,
  ) = body
  should.be_true(True)
}

// ---------------------------------------------------------------------------
// Lambda
// ---------------------------------------------------------------------------

// The default input variable is x, so lambda parameters should use a different
// name. Here we use y as the explicit parameter.
pub fn parse_lambda_test() {
  let body = single_body("f: (y: z)")
  let assert parser.PipeExpr(
    parser.PrefixExpr(
      parser.AccessExpr(parser.SExprLit(parser.LambdaBody("y", inner)), _),
      _,
    ),
    _,
  ) = body
  inner
  |> simple_primary
  |> should.equal(parser.NameRef("z"))
}

// ---------------------------------------------------------------------------
// Array literal
// ---------------------------------------------------------------------------

pub fn parse_array_test() {
  let body = single_body("z: [1, 2, 3]")
  let assert parser.PipeExpr(
    parser.PrefixExpr(parser.AccessExpr(parser.ArrayLit(entries), _), _),
    _,
  ) = body
  list.length(entries)
  |> should.equal(3)
}

pub fn parse_empty_array_test() {
  single_body("z: []")
  |> simple_primary
  |> should.equal(parser.ArrayLit([]))
}

// ---------------------------------------------------------------------------
// Tuple literal
// ---------------------------------------------------------------------------

pub fn parse_tuple_test() {
  let body = single_body("z: { a: 1, b: 2 }")
  let assert parser.PipeExpr(
    parser.PrefixExpr(
      parser.AccessExpr(parser.TupleLit(entries), _),
      _,
    ),
    _,
  ) = body
  list.length(entries)
  |> should.equal(2)
}

pub fn parse_empty_tuple_test() {
  single_body("z: {}")
  |> simple_primary
  |> should.equal(parser.TupleLit([]))
}

// ---------------------------------------------------------------------------
// Conditional map
// ---------------------------------------------------------------------------

pub fn parse_conditional_map_test() {
  let body = single_body("z: [(a): 1, (b): 2]")
  let assert parser.PipeExpr(
    parser.PrefixExpr(
      parser.AccessExpr(parser.ConditionalMapLit(entries), _),
      _,
    ),
    _,
  ) = body
  list.length(entries)
  |> should.equal(2)
}

// ---------------------------------------------------------------------------
// Type annotations
// ---------------------------------------------------------------------------

// Nominal (identifier) types are not allowed on definitions
pub fn parse_identifier_type_annotation_test() {
  parse_src("z *Foo: 42")
  |> should.be_error()
}

pub fn parse_alias_type_annotation_test() {
  let assert Ok(parser.Program([parser.Definition(_, annot, _)])) =
    parse_src("z 'Foo: 42")
  annot
  |> should.equal(Some(parser.AliasType(parser.TypeLabel("Foo"))))
}

pub fn parse_no_annotation_test() {
  let assert Ok(parser.Program([parser.Definition(_, annot, _)])) =
    parse_src("z: 42")
  annot
  |> should.equal(None)
}

// Nominal types with function syntax are also disallowed on definitions
pub fn parse_type_function_annotation_test() {
  parse_src("f *(Int | Str): 42")
  |> should.be_error()
}

// ---------------------------------------------------------------------------
// Multiple definitions
// ---------------------------------------------------------------------------

pub fn parse_multiple_definitions_test() {
  let assert Ok(parser.Program(defs)) = parse_src("a: 1\nb: 2")
  list.length(defs)
  |> should.equal(2)
  let assert [parser.Definition(name1, _, _), parser.Definition(name2, _, _)] =
    defs
  name1 |> should.equal("a")
  name2 |> should.equal("b")
}

// ---------------------------------------------------------------------------
// Empty program
// ---------------------------------------------------------------------------

pub fn parse_empty_program_test() {
  parse_src("")
  |> should.equal(Ok(parser.Program([])))
}
