import XCTest
@testable import WeLangLib

final class ParserTests: XCTestCase {

    private func parseSource(_ source: String) throws -> Program {
        let tokens = try lex(source)
        return try parse(tokens)
    }

    // MARK: - Scalar Definitions

    func testParseIntegerDefinition() throws {
        let program = try parseSource("zero: 0")
        XCTAssertEqual(program.definitions.count, 1)
        let def = program.definitions[0]
        XCTAssertEqual(def.label, "zero")
        XCTAssertNil(def.typeAnnotation)
        guard case .integerLiteral(let val, _) = def.value else {
            XCTFail("Expected integerLiteral, got \(def.value)"); return
        }
        XCTAssertEqual(val, "0")
    }

    func testParseNegativeIntegerDefinition() throws {
        let program = try parseSource("neg: -1")
        XCTAssertEqual(program.definitions.count, 1)
        guard case .integerLiteral(let val, _) = program.definitions[0].value else {
            XCTFail("Expected integerLiteral"); return
        }
        XCTAssertEqual(val, "-1")
    }

    func testParseFloatDefinition() throws {
        let program = try parseSource("pi: 3.14")
        XCTAssertEqual(program.definitions.count, 1)
        guard case .floatLiteral(let val, _) = program.definitions[0].value else {
            XCTFail("Expected floatLiteral"); return
        }
        XCTAssertEqual(val, "3.14")
    }

    func testParseStringDefinition() throws {
        let program = try parseSource(#"name: "alice""#)
        XCTAssertEqual(program.definitions.count, 1)
        guard case .stringLiteral(let val, _) = program.definitions[0].value else {
            XCTFail("Expected stringLiteral"); return
        }
        XCTAssertEqual(val, "alice")
    }

    func testParseInterpolatedStringDefinition() throws {
        let program = try parseSource("greeting: `hello {{name}}`")
        XCTAssertEqual(program.definitions.count, 1)
        guard case .interpolatedStringLiteral(_, _) = program.definitions[0].value else {
            XCTFail("Expected interpolatedStringLiteral"); return
        }
    }

    // MARK: - Typed Definitions

    func testParseTypedDefinition() throws {
        let program = try parseSource("anInt u32: 23")
        XCTAssertEqual(program.definitions.count, 1)
        let def = program.definitions[0]
        XCTAssertEqual(def.label, "anInt")
        guard case .name(let typeName, _) = def.typeAnnotation else {
            XCTFail("Expected type annotation .name(\"u32\", _)"); return
        }
        XCTAssertEqual(typeName, "u32")
        guard case .integerLiteral(let val, _) = def.value else {
            XCTFail("Expected integerLiteral(\"23\", _)"); return
        }
        XCTAssertEqual(val, "23")
    }

    func testParseTypedDefinitionFloat() throws {
        let program = try parseSource("pi f64: 3.14")
        XCTAssertEqual(program.definitions.count, 1)
        let def = program.definitions[0]
        guard case .name(let typeName, _) = def.typeAnnotation else {
            XCTFail("Expected type annotation .name(\"f64\", _)"); return
        }
        XCTAssertEqual(typeName, "f64")
    }

    func testParseUntypedDefinition() throws {
        let program = try parseSource("zero: 0")
        XCTAssertEqual(program.definitions.count, 1)
        XCTAssertNil(program.definitions[0].typeAnnotation)
    }

    // MARK: - Names and Discard

    func testParseNameReference() throws {
        let program = try parseSource("alias: other")
        XCTAssertEqual(program.definitions.count, 1)
        guard case .name(let val, _) = program.definitions[0].value else {
            XCTFail("Expected name"); return
        }
        XCTAssertEqual(val, "other")
    }

    func testParseImplicitInput() throws {
        let program = try parseSource("echo: x")
        XCTAssertEqual(program.definitions.count, 1)
        guard case .name(let val, _) = program.definitions[0].value else {
            XCTFail("Expected name(\"x\", _)"); return
        }
        XCTAssertEqual(val, "x")
    }

    func testParseDiscard() throws {
        let program = try parseSource("ignore: _")
        XCTAssertEqual(program.definitions.count, 1)
        guard case .discard(_) = program.definitions[0].value else {
            XCTFail("Expected discard"); return
        }
    }

    // MARK: - Unit

    func testParseUnit() throws {
        let program = try parseSource("blank: ()")
        XCTAssertEqual(program.definitions.count, 1)
        guard case .unit(_) = program.definitions[0].value else {
            XCTFail("Expected unit"); return
        }
    }

    // MARK: - Multiple Definitions

    func testParseMultipleDefinitions() throws {
        let program = try parseSource("a: 1\nb: 2")
        XCTAssertEqual(program.definitions.count, 2)
        XCTAssertEqual(program.definitions[0].label, "a")
        XCTAssertEqual(program.definitions[1].label, "b")
    }

    func testParseMultipleDefinitionsWithBlankLines() throws {
        let program = try parseSource("a: 1\n\n\nb: 2")
        XCTAssertEqual(program.definitions.count, 2)
        XCTAssertEqual(program.definitions[0].label, "a")
        XCTAssertEqual(program.definitions[1].label, "b")
    }

    func testParseMultipleDefinitionsSameLine() throws {
        let program = try parseSource("foo: 1 bar: 2")
        XCTAssertEqual(program.definitions.count, 2)
        XCTAssertEqual(program.definitions[0].label, "foo")
        XCTAssertEqual(program.definitions[1].label, "bar")
    }

    // MARK: - Error Cases

    func testParseMissingColon() throws {
        XCTAssertThrowsError(try parseSource("foo 0")) { error in
            guard case ParseError.expectedColon(_) = error else {
                XCTFail("Expected expectedColon, got \(error)"); return
            }
        }
    }

    func testParseMissingValueEof() throws {
        XCTAssertThrowsError(try parseSource("foo:")) { error in
            guard case ParseError.expectedExpression(_) = error else {
                XCTFail("Expected expectedExpression, got \(error)"); return
            }
        }
    }

    func testParseMissingValueNewline() throws {
        XCTAssertThrowsError(try parseSource("foo:\n")) { error in
            guard case ParseError.expectedExpression(_) = error else {
                XCTFail("Expected expectedExpression, got \(error)"); return
            }
        }
    }

    func testParseBareExpression() throws {
        XCTAssertThrowsError(try parseSource("42")) { error in
            guard case ParseError.expectedDefinition(_) = error else {
                XCTFail("Expected expectedDefinition, got \(error)"); return
            }
        }
    }

    // MARK: - Whitespace Independence

    func testParseDefinitionValueOnNextLine() throws {
        let program = try parseSource("foo:\n0")
        XCTAssertEqual(program.definitions.count, 1)
        guard case .integerLiteral(let val, _) = program.definitions[0].value else {
            XCTFail("Expected integerLiteral"); return
        }
        XCTAssertEqual(val, "0")
    }

    func testParseDefinitionSpreadAcrossLines() throws {
        let program = try parseSource("foo\n:\n0")
        XCTAssertEqual(program.definitions.count, 1)
        XCTAssertEqual(program.definitions[0].label, "foo")
        guard case .integerLiteral(let val, _) = program.definitions[0].value else {
            XCTFail("Expected integerLiteral"); return
        }
        XCTAssertEqual(val, "0")
    }

    func testParseTypedDefinitionAcrossLines() throws {
        let program = try parseSource("anInt\nu32\n:\n23")
        XCTAssertEqual(program.definitions.count, 1)
        let def = program.definitions[0]
        XCTAssertEqual(def.label, "anInt")
        guard case .name(let typeName, _) = def.typeAnnotation else {
            XCTFail("Expected type annotation .name(\"u32\", _)"); return
        }
        XCTAssertEqual(typeName, "u32")
    }

    // MARK: - Edge Cases

    func testParseEmptySource() throws {
        let program = try parseSource("")
        XCTAssertEqual(program.definitions.count, 0)
    }

    func testParseOnlyNewlines() throws {
        let program = try parseSource("\n\n\n")
        XCTAssertEqual(program.definitions.count, 0)
    }

    func testParseTrailingNewline() throws {
        let program = try parseSource("x: 1\n")
        XCTAssertEqual(program.definitions.count, 1)
        XCTAssertEqual(program.definitions[0].label, "x")
    }

    // MARK: - Basic S-expressions

    func testParseSingleElementParen() throws {
        let program = try parseSource("id: (x)")
        XCTAssertEqual(program.definitions.count, 1)
        guard case .name(let val, _) = program.definitions[0].value else {
            XCTFail("Expected .name(\"x\", _), got \(program.definitions[0].value)"); return
        }
        XCTAssertEqual(val, "x")
    }

    func testParseUnitExpr() throws {
        let program = try parseSource("u: ()")
        XCTAssertEqual(program.definitions.count, 1)
        guard case .unit(_) = program.definitions[0].value else {
            XCTFail("Expected .unit(_), got \(program.definitions[0].value)"); return
        }
    }

    func testParseApplyOneArg() throws {
        let program = try parseSource("r: (increment 1)")
        XCTAssertEqual(program.definitions.count, 1)
        guard case .apply(let fn, let args, _) = program.definitions[0].value else {
            XCTFail("Expected .apply, got \(program.definitions[0].value)"); return
        }
        guard case .name(let fnName, _) = fn else {
            XCTFail("Expected .name for function, got \(fn)"); return
        }
        XCTAssertEqual(fnName, "increment")
        XCTAssertEqual(args.count, 1)
        guard case .integerLiteral(let v, _) = args[0] else {
            XCTFail("Expected integerLiteral arg, got \(args[0])"); return
        }
        XCTAssertEqual(v, "1")
    }

    func testParseApplyTwoArgs() throws {
        let program = try parseSource("r: (add 1 2)")
        XCTAssertEqual(program.definitions.count, 1)
        guard case .apply(let fn, let args, _) = program.definitions[0].value else {
            XCTFail("Expected .apply, got \(program.definitions[0].value)"); return
        }
        guard case .name(let fnName, _) = fn else {
            XCTFail("Expected .name for function"); return
        }
        XCTAssertEqual(fnName, "add")
        XCTAssertEqual(args.count, 2)
        guard case .integerLiteral(let v1, _) = args[0],
              case .integerLiteral(let v2, _) = args[1] else {
            XCTFail("Expected integerLiteral args"); return
        }
        XCTAssertEqual(v1, "1")
        XCTAssertEqual(v2, "2")
    }

    // MARK: - Nested S-expressions

    func testParseNestedApply() throws {
        let program = try parseSource("r: (add (multiply 2 3) 4)")
        XCTAssertEqual(program.definitions.count, 1)
        guard case .apply(let fn, let args, _) = program.definitions[0].value else {
            XCTFail("Expected outer .apply"); return
        }
        guard case .name(let fnName, _) = fn else {
            XCTFail("Expected .name for outer function"); return
        }
        XCTAssertEqual(fnName, "add")
        XCTAssertEqual(args.count, 2)
        guard case .apply(let innerFn, let innerArgs, _) = args[0] else {
            XCTFail("Expected inner .apply for first arg, got \(args[0])"); return
        }
        guard case .name(let innerFnName, _) = innerFn else {
            XCTFail("Expected .name for inner function"); return
        }
        XCTAssertEqual(innerFnName, "multiply")
        XCTAssertEqual(innerArgs.count, 2)
        guard case .integerLiteral("4", _) = args[1] else {
            XCTFail("Expected integerLiteral(\"4\") for second arg, got \(args[1])"); return
        }
    }

    func testParseDeeplyNested() throws {
        let program = try parseSource("r: (f (g (h 1)))")
        XCTAssertEqual(program.definitions.count, 1)
        guard case .apply(let fn, let args, _) = program.definitions[0].value else {
            XCTFail("Expected outer .apply"); return
        }
        guard case .name("f", _) = fn else { XCTFail("Expected .name(\"f\")"); return }
        XCTAssertEqual(args.count, 1)
        guard case .apply(let fn2, let args2, _) = args[0] else {
            XCTFail("Expected second-level .apply"); return
        }
        guard case .name("g", _) = fn2 else { XCTFail("Expected .name(\"g\")"); return }
        XCTAssertEqual(args2.count, 1)
        guard case .apply(let fn3, let args3, _) = args2[0] else {
            XCTFail("Expected third-level .apply"); return
        }
        guard case .name("h", _) = fn3 else { XCTFail("Expected .name(\"h\")"); return }
        XCTAssertEqual(args3.count, 1)
        guard case .integerLiteral("1", _) = args3[0] else {
            XCTFail("Expected integerLiteral(\"1\")"); return
        }
    }

    // MARK: - Pipe expressions

    func testParsePipeTwoClauses() throws {
        let program = try parseSource("r: (1 | increment)")
        XCTAssertEqual(program.definitions.count, 1)
        guard case .pipe(let clauses, _) = program.definitions[0].value else {
            XCTFail("Expected .pipe, got \(program.definitions[0].value)"); return
        }
        XCTAssertEqual(clauses.count, 2)
        guard case .integerLiteral("1", _) = clauses[0] else {
            XCTFail("Expected integerLiteral(\"1\") as first clause"); return
        }
        guard case .name("increment", _) = clauses[1] else {
            XCTFail("Expected .name(\"increment\") as second clause"); return
        }
    }

    func testParsePipeThreeClauses() throws {
        let program = try parseSource("r: (1 | add 2 | multiply 3)")
        XCTAssertEqual(program.definitions.count, 1)
        guard case .pipe(let clauses, _) = program.definitions[0].value else {
            XCTFail("Expected .pipe"); return
        }
        XCTAssertEqual(clauses.count, 3)
        guard case .integerLiteral("1", _) = clauses[0] else {
            XCTFail("Expected integerLiteral(\"1\") as clause 0"); return
        }
        guard case .apply(let fn1, let args1, _) = clauses[1] else {
            XCTFail("Expected .apply as clause 1"); return
        }
        guard case .name("add", _) = fn1 else { XCTFail("Expected .name(\"add\")"); return }
        XCTAssertEqual(args1.count, 1)
        guard case .integerLiteral("2", _) = args1[0] else {
            XCTFail("Expected integerLiteral(\"2\")"); return
        }
        guard case .apply(let fn2, let args2, _) = clauses[2] else {
            XCTFail("Expected .apply as clause 2"); return
        }
        guard case .name("multiply", _) = fn2 else { XCTFail("Expected .name(\"multiply\")"); return }
        XCTAssertEqual(args2.count, 1)
        guard case .integerLiteral("3", _) = args2[0] else {
            XCTFail("Expected integerLiteral(\"3\")"); return
        }
    }

    func testParsePipeSingleTokenClauses() throws {
        let program = try parseSource("r: (1 | 2 | 3)")
        XCTAssertEqual(program.definitions.count, 1)
        guard case .pipe(let clauses, _) = program.definitions[0].value else {
            XCTFail("Expected .pipe"); return
        }
        XCTAssertEqual(clauses.count, 3)
        guard case .integerLiteral("1", _) = clauses[0],
              case .integerLiteral("2", _) = clauses[1],
              case .integerLiteral("3", _) = clauses[2] else {
            XCTFail("Expected three integerLiteral clauses"); return
        }
    }

    // MARK: - Leading pipe

    func testParseLeadingPipe() throws {
        let program = try parseSource("f: (| increment)")
        XCTAssertEqual(program.definitions.count, 1)
        guard case .pipe(let clauses, _) = program.definitions[0].value else {
            XCTFail("Expected .pipe, got \(program.definitions[0].value)"); return
        }
        XCTAssertEqual(clauses.count, 2)
        guard case .name("x", _) = clauses[0] else {
            XCTFail("Expected implicit .name(\"x\") as first clause, got \(clauses[0])"); return
        }
        guard case .name("increment", _) = clauses[1] else {
            XCTFail("Expected .name(\"increment\") as second clause"); return
        }
    }

    func testParseLeadingPipeMultiple() throws {
        let program = try parseSource("f: (| add 1 | double)")
        XCTAssertEqual(program.definitions.count, 1)
        guard case .pipe(let clauses, _) = program.definitions[0].value else {
            XCTFail("Expected .pipe"); return
        }
        XCTAssertEqual(clauses.count, 3)
        guard case .name("x", _) = clauses[0] else {
            XCTFail("Expected implicit x as first clause"); return
        }
        guard case .apply(let fn, _, _) = clauses[1] else {
            XCTFail("Expected .apply as second clause"); return
        }
        guard case .name("add", _) = fn else { XCTFail("Expected .name(\"add\")"); return }
        guard case .name("double", _) = clauses[2] else {
            XCTFail("Expected .name(\"double\") as third clause"); return
        }
    }

    // MARK: - Multi-line expressions

    func testParseMultilineSExpr() throws {
        let source = "r: (\n  add\n  1\n  2\n)"
        let program = try parseSource(source)
        XCTAssertEqual(program.definitions.count, 1)
        guard case .apply(let fn, let args, _) = program.definitions[0].value else {
            XCTFail("Expected .apply, got \(program.definitions[0].value)"); return
        }
        guard case .name("add", _) = fn else { XCTFail("Expected .name(\"add\")"); return }
        XCTAssertEqual(args.count, 2)
        guard case .integerLiteral("1", _) = args[0],
              case .integerLiteral("2", _) = args[1] else {
            XCTFail("Expected integerLiteral args"); return
        }
    }

    func testParseMultilinePipe() throws {
        let source = "r: (\n  1\n  | add 2\n  | multiply 3\n)"
        let program = try parseSource(source)
        XCTAssertEqual(program.definitions.count, 1)
        guard case .pipe(let clauses, _) = program.definitions[0].value else {
            XCTFail("Expected .pipe"); return
        }
        XCTAssertEqual(clauses.count, 3)
        guard case .integerLiteral("1", _) = clauses[0] else {
            XCTFail("Expected integerLiteral(\"1\") as first clause"); return
        }
        guard case .apply(_, _, _) = clauses[1] else {
            XCTFail("Expected .apply as second clause"); return
        }
        guard case .apply(_, _, _) = clauses[2] else {
            XCTFail("Expected .apply as third clause"); return
        }
    }

    // MARK: - Mixed prefix/postfix

    func testParseMixedPrefixPostfix() throws {
        let program = try parseSource("r: (1 | 3 2 | 6 5 4)")
        XCTAssertEqual(program.definitions.count, 1)
        guard case .pipe(let clauses, _) = program.definitions[0].value else {
            XCTFail("Expected .pipe"); return
        }
        XCTAssertEqual(clauses.count, 3)
        guard case .integerLiteral("1", _) = clauses[0] else {
            XCTFail("Expected integerLiteral(\"1\") as clause 0"); return
        }
        guard case .apply(let fn1, let args1, _) = clauses[1] else {
            XCTFail("Expected .apply as clause 1"); return
        }
        guard case .integerLiteral("3", _) = fn1 else {
            XCTFail("Expected integerLiteral(\"3\") as function in clause 1"); return
        }
        XCTAssertEqual(args1.count, 1)
        guard case .integerLiteral("2", _) = args1[0] else {
            XCTFail("Expected integerLiteral(\"2\") as arg in clause 1"); return
        }
        guard case .apply(let fn2, let args2, _) = clauses[2] else {
            XCTFail("Expected .apply as clause 2"); return
        }
        guard case .integerLiteral("6", _) = fn2 else {
            XCTFail("Expected integerLiteral(\"6\") as function in clause 2"); return
        }
        XCTAssertEqual(args2.count, 2)
    }

    func testParseNumberAsFunction() throws {
        let program = try parseSource("r: (3 2 1)")
        XCTAssertEqual(program.definitions.count, 1)
        guard case .apply(let fn, let args, _) = program.definitions[0].value else {
            XCTFail("Expected .apply"); return
        }
        guard case .integerLiteral("3", _) = fn else {
            XCTFail("Expected integerLiteral(\"3\") as function"); return
        }
        XCTAssertEqual(args.count, 2)
        guard case .integerLiteral("2", _) = args[0],
              case .integerLiteral("1", _) = args[1] else {
            XCTFail("Expected integerLiteral args 2 and 1"); return
        }
    }

    // MARK: - Lambda with named parameter

    func testParseLambdaSimple() throws {
        let program = try parseSource("f: (it: it)")
        XCTAssertEqual(program.definitions.count, 1)
        guard case .lambda(let param, let body, _) = program.definitions[0].value else {
            XCTFail("Expected .lambda, got \(program.definitions[0].value)"); return
        }
        XCTAssertEqual(param, "it")
        guard case .name("it", _) = body else {
            XCTFail("Expected .name(\"it\") as body, got \(body)"); return
        }
    }

    func testParseLambdaWithApply() throws {
        let program = try parseSource("f: (it: do it)")
        XCTAssertEqual(program.definitions.count, 1)
        guard case .lambda(let param, let body, _) = program.definitions[0].value else {
            XCTFail("Expected .lambda"); return
        }
        XCTAssertEqual(param, "it")
        guard case .apply(let fn, let args, _) = body else {
            XCTFail("Expected .apply as lambda body, got \(body)"); return
        }
        guard case .name("do", _) = fn else { XCTFail("Expected .name(\"do\")"); return }
        XCTAssertEqual(args.count, 1)
        guard case .name("it", _) = args[0] else {
            XCTFail("Expected .name(\"it\") as arg"); return
        }
    }

    func testParseLambdaWithPipe() throws {
        let program = try parseSource("f: (it: it | double | increment)")
        XCTAssertEqual(program.definitions.count, 1)
        guard case .lambda(let param, let body, _) = program.definitions[0].value else {
            XCTFail("Expected .lambda"); return
        }
        XCTAssertEqual(param, "it")
        guard case .pipe(let clauses, _) = body else {
            XCTFail("Expected .pipe as lambda body, got \(body)"); return
        }
        XCTAssertEqual(clauses.count, 3)
        guard case .name("it", _) = clauses[0],
              case .name("double", _) = clauses[1],
              case .name("increment", _) = clauses[2] else {
            XCTFail("Expected three name clauses in pipe"); return
        }
    }

    func testParseLambdaAsArgument() throws {
        let program = try parseSource("r: (something (it: do it) x)")
        XCTAssertEqual(program.definitions.count, 1)
        guard case .apply(let fn, let args, _) = program.definitions[0].value else {
            XCTFail("Expected outer .apply"); return
        }
        guard case .name("something", _) = fn else {
            XCTFail("Expected .name(\"something\")"); return
        }
        XCTAssertEqual(args.count, 2)
        guard case .lambda(let param, _, _) = args[0] else {
            XCTFail("Expected .lambda as first arg, got \(args[0])"); return
        }
        XCTAssertEqual(param, "it")
        guard case .name("x", _) = args[1] else {
            XCTFail("Expected .name(\"x\") as second arg"); return
        }
    }

    func testParseLambdaNestedInPipe() throws {
        let program = try parseSource("r: (data | (item: transform item))")
        XCTAssertEqual(program.definitions.count, 1)
        guard case .pipe(let clauses, _) = program.definitions[0].value else {
            XCTFail("Expected .pipe"); return
        }
        XCTAssertEqual(clauses.count, 2)
        guard case .name("data", _) = clauses[0] else {
            XCTFail("Expected .name(\"data\") as first clause"); return
        }
        guard case .lambda(let param, _, _) = clauses[1] else {
            XCTFail("Expected .lambda as second clause, got \(clauses[1])"); return
        }
        XCTAssertEqual(param, "item")
    }

    func testParseLambdaDifferentName() throws {
        let program = try parseSource("f: (val: process val)")
        XCTAssertEqual(program.definitions.count, 1)
        guard case .lambda(let param, let body, _) = program.definitions[0].value else {
            XCTFail("Expected .lambda"); return
        }
        XCTAssertEqual(param, "val")
        guard case .apply(let fn, let args, _) = body else {
            XCTFail("Expected .apply as body"); return
        }
        guard case .name("process", _) = fn else { XCTFail("Expected .name(\"process\")"); return }
        XCTAssertEqual(args.count, 1)
        guard case .name("val", _) = args[0] else {
            XCTFail("Expected .name(\"val\") as arg"); return
        }
    }

    // MARK: - Error cases

    func testParseMissingClosingParen() throws {
        XCTAssertThrowsError(try parseSource("r: (add 1")) { error in
            guard case ParseError.expectedClosingParen(_) = error else {
                XCTFail("Expected expectedClosingParen, got \(error)"); return
            }
        }
    }

    func testParseEmptyClause() throws {
        XCTAssertThrowsError(try parseSource("r: (1 | | 2)")) { error in
            guard case ParseError.emptyClause(_) = error else {
                XCTFail("Expected emptyClause, got \(error)"); return
            }
        }
    }
}
