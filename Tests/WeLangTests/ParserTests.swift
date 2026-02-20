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

    // MARK: - Basic S-Expressions

    func testParseSingleElementParen() throws {
        let program = try parseSource("id: (x)")
        XCTAssertEqual(program.definitions.count, 1)
        let def = program.definitions[0]
        XCTAssertEqual(def.label, "id")
        guard case .name(let val, _) = def.value else {
            XCTFail("Expected name, got \(def.value)"); return
        }
        XCTAssertEqual(val, "x")
    }

    func testParseUnitExpr() throws {
        let program = try parseSource("u: ()")
        XCTAssertEqual(program.definitions.count, 1)
        guard case .unit(_) = program.definitions[0].value else {
            XCTFail("Expected unit"); return
        }
    }

    func testParseApplyOneArg() throws {
        let program = try parseSource("r: (increment 1)")
        XCTAssertEqual(program.definitions.count, 1)
        guard case .apply(let function, let arguments, _) = program.definitions[0].value else {
            XCTFail("Expected apply, got \(program.definitions[0].value)"); return
        }
        guard case .name("increment", _) = function else {
            XCTFail("Expected function name 'increment', got \(function)"); return
        }
        XCTAssertEqual(arguments.count, 1)
        guard case .integerLiteral("1", _) = arguments[0] else {
            XCTFail("Expected integerLiteral '1'"); return
        }
    }

    func testParseApplyTwoArgs() throws {
        let program = try parseSource("r: (add 1 2)")
        XCTAssertEqual(program.definitions.count, 1)
        guard case .apply(let function, let arguments, _) = program.definitions[0].value else {
            XCTFail("Expected apply"); return
        }
        guard case .name("add", _) = function else {
            XCTFail("Expected function name 'add'"); return
        }
        XCTAssertEqual(arguments.count, 2)
        guard case .integerLiteral("1", _) = arguments[0] else {
            XCTFail("Expected integerLiteral '1'"); return
        }
        guard case .integerLiteral("2", _) = arguments[1] else {
            XCTFail("Expected integerLiteral '2'"); return
        }
    }

    // MARK: - Nested S-Expressions

    func testParseNestedApply() throws {
        let program = try parseSource("r: (add (multiply 2 3) 4)")
        XCTAssertEqual(program.definitions.count, 1)
        guard case .apply(let function, let arguments, _) = program.definitions[0].value else {
            XCTFail("Expected apply"); return
        }
        guard case .name("add", _) = function else {
            XCTFail("Expected 'add'"); return
        }
        XCTAssertEqual(arguments.count, 2)
        // First argument is nested apply
        guard case .apply(let innerFn, let innerArgs, _) = arguments[0] else {
            XCTFail("Expected nested apply, got \(arguments[0])"); return
        }
        guard case .name("multiply", _) = innerFn else {
            XCTFail("Expected 'multiply'"); return
        }
        XCTAssertEqual(innerArgs.count, 2)
        // Second argument is literal
        guard case .integerLiteral("4", _) = arguments[1] else {
            XCTFail("Expected integerLiteral '4'"); return
        }
    }

    func testParseDeeplyNested() throws {
        let program = try parseSource("r: (f (g (h 1)))")
        XCTAssertEqual(program.definitions.count, 1)
        guard case .apply(let f, let fArgs, _) = program.definitions[0].value else {
            XCTFail("Expected apply"); return
        }
        guard case .name("f", _) = f else { XCTFail("Expected 'f'"); return }
        XCTAssertEqual(fArgs.count, 1)
        guard case .apply(let g, let gArgs, _) = fArgs[0] else {
            XCTFail("Expected nested apply for g"); return
        }
        guard case .name("g", _) = g else { XCTFail("Expected 'g'"); return }
        XCTAssertEqual(gArgs.count, 1)
        guard case .apply(let h, let hArgs, _) = gArgs[0] else {
            XCTFail("Expected nested apply for h"); return
        }
        guard case .name("h", _) = h else { XCTFail("Expected 'h'"); return }
        XCTAssertEqual(hArgs.count, 1)
        guard case .integerLiteral("1", _) = hArgs[0] else {
            XCTFail("Expected integerLiteral '1'"); return
        }
    }

    // MARK: - Pipe Expressions

    func testParsePipeTwoClauses() throws {
        let program = try parseSource("r: (1 | increment)")
        XCTAssertEqual(program.definitions.count, 1)
        guard case .pipe(let clauses, _) = program.definitions[0].value else {
            XCTFail("Expected pipe, got \(program.definitions[0].value)"); return
        }
        XCTAssertEqual(clauses.count, 2)
        guard case .integerLiteral("1", _) = clauses[0] else {
            XCTFail("Expected integerLiteral '1'"); return
        }
        guard case .name("increment", _) = clauses[1] else {
            XCTFail("Expected name 'increment'"); return
        }
    }

    func testParsePipeThreeClauses() throws {
        let program = try parseSource("r: (1 | add 2 | multiply 3)")
        XCTAssertEqual(program.definitions.count, 1)
        guard case .pipe(let clauses, _) = program.definitions[0].value else {
            XCTFail("Expected pipe"); return
        }
        XCTAssertEqual(clauses.count, 3)
        guard case .integerLiteral("1", _) = clauses[0] else {
            XCTFail("Expected integerLiteral '1'"); return
        }
        guard case .apply(let fn1, let args1, _) = clauses[1] else {
            XCTFail("Expected apply for second clause"); return
        }
        guard case .name("add", _) = fn1 else { XCTFail("Expected 'add'"); return }
        XCTAssertEqual(args1.count, 1)
        guard case .apply(let fn2, let args2, _) = clauses[2] else {
            XCTFail("Expected apply for third clause"); return
        }
        guard case .name("multiply", _) = fn2 else { XCTFail("Expected 'multiply'"); return }
        XCTAssertEqual(args2.count, 1)
    }

    func testParsePipeSingleTokenClauses() throws {
        let program = try parseSource("r: (1 | 2 | 3)")
        XCTAssertEqual(program.definitions.count, 1)
        guard case .pipe(let clauses, _) = program.definitions[0].value else {
            XCTFail("Expected pipe"); return
        }
        XCTAssertEqual(clauses.count, 3)
        guard case .integerLiteral("1", _) = clauses[0] else { XCTFail("Expected '1'"); return }
        guard case .integerLiteral("2", _) = clauses[1] else { XCTFail("Expected '2'"); return }
        guard case .integerLiteral("3", _) = clauses[2] else { XCTFail("Expected '3'"); return }
    }

    // MARK: - Leading Pipe

    func testParseLeadingPipe() throws {
        let program = try parseSource("f: (| increment)")
        XCTAssertEqual(program.definitions.count, 1)
        guard case .pipe(let clauses, _) = program.definitions[0].value else {
            XCTFail("Expected pipe, got \(program.definitions[0].value)"); return
        }
        XCTAssertEqual(clauses.count, 2)
        guard case .name("x", _) = clauses[0] else {
            XCTFail("Expected implicit name 'x', got \(clauses[0])"); return
        }
        guard case .name("increment", _) = clauses[1] else {
            XCTFail("Expected name 'increment'"); return
        }
    }

    func testParseLeadingPipeMultiple() throws {
        let program = try parseSource("f: (| add 1 | double)")
        XCTAssertEqual(program.definitions.count, 1)
        guard case .pipe(let clauses, _) = program.definitions[0].value else {
            XCTFail("Expected pipe"); return
        }
        XCTAssertEqual(clauses.count, 3)
        guard case .name("x", _) = clauses[0] else {
            XCTFail("Expected implicit 'x'"); return
        }
        guard case .apply(let fn, let args, _) = clauses[1] else {
            XCTFail("Expected apply for 'add 1'"); return
        }
        guard case .name("add", _) = fn else { XCTFail("Expected 'add'"); return }
        XCTAssertEqual(args.count, 1)
        guard case .name("double", _) = clauses[2] else {
            XCTFail("Expected name 'double'"); return
        }
    }

    // MARK: - Multi-line Expressions

    func testParseMultilineSExpr() throws {
        let program = try parseSource("r: (\n  add\n  1\n  2\n)")
        XCTAssertEqual(program.definitions.count, 1)
        guard case .apply(let function, let arguments, _) = program.definitions[0].value else {
            XCTFail("Expected apply"); return
        }
        guard case .name("add", _) = function else {
            XCTFail("Expected 'add'"); return
        }
        XCTAssertEqual(arguments.count, 2)
        guard case .integerLiteral("1", _) = arguments[0] else {
            XCTFail("Expected '1'"); return
        }
        guard case .integerLiteral("2", _) = arguments[1] else {
            XCTFail("Expected '2'"); return
        }
    }

    func testParseMultilinePipe() throws {
        let program = try parseSource("r: (\n  1\n  | add 2\n  | multiply 3\n)")
        XCTAssertEqual(program.definitions.count, 1)
        guard case .pipe(let clauses, _) = program.definitions[0].value else {
            XCTFail("Expected pipe, got \(program.definitions[0].value)"); return
        }
        XCTAssertEqual(clauses.count, 3)
        guard case .integerLiteral("1", _) = clauses[0] else {
            XCTFail("Expected '1'"); return
        }
        guard case .apply(_, _, _) = clauses[1] else {
            XCTFail("Expected apply for 'add 2'"); return
        }
        guard case .apply(_, _, _) = clauses[2] else {
            XCTFail("Expected apply for 'multiply 3'"); return
        }
    }

    // MARK: - Mixed Prefix/Postfix

    func testParseMixedPrefixPostfix() throws {
        let program = try parseSource("r: (1 | 3 2 | 6 5 4)")
        XCTAssertEqual(program.definitions.count, 1)
        guard case .pipe(let clauses, _) = program.definitions[0].value else {
            XCTFail("Expected pipe"); return
        }
        XCTAssertEqual(clauses.count, 3)
        guard case .integerLiteral("1", _) = clauses[0] else {
            XCTFail("Expected '1'"); return
        }
        guard case .apply(let fn1, let args1, _) = clauses[1] else {
            XCTFail("Expected apply for '3 2'"); return
        }
        guard case .integerLiteral("3", _) = fn1 else { XCTFail("Expected '3'"); return }
        XCTAssertEqual(args1.count, 1)
        guard case .apply(let fn2, let args2, _) = clauses[2] else {
            XCTFail("Expected apply for '6 5 4'"); return
        }
        guard case .integerLiteral("6", _) = fn2 else { XCTFail("Expected '6'"); return }
        XCTAssertEqual(args2.count, 2)
    }

    func testParseNumberAsFunction() throws {
        let program = try parseSource("r: (3 2 1)")
        XCTAssertEqual(program.definitions.count, 1)
        guard case .apply(let function, let arguments, _) = program.definitions[0].value else {
            XCTFail("Expected apply"); return
        }
        guard case .integerLiteral("3", _) = function else {
            XCTFail("Expected '3' as function"); return
        }
        XCTAssertEqual(arguments.count, 2)
        guard case .integerLiteral("2", _) = arguments[0] else { XCTFail("Expected '2'"); return }
        guard case .integerLiteral("1", _) = arguments[1] else { XCTFail("Expected '1'"); return }
    }

    // MARK: - Lambda with Named Parameter

    func testParseLambdaSimple() throws {
        let program = try parseSource("f: (it: it)")
        XCTAssertEqual(program.definitions.count, 1)
        guard case .lambda(let param, let body, _) = program.definitions[0].value else {
            XCTFail("Expected lambda, got \(program.definitions[0].value)"); return
        }
        XCTAssertEqual(param, "it")
        guard case .name("it", _) = body else {
            XCTFail("Expected name 'it' as body"); return
        }
    }

    func testParseLambdaWithApply() throws {
        let program = try parseSource("f: (it: do it)")
        XCTAssertEqual(program.definitions.count, 1)
        guard case .lambda(let param, let body, _) = program.definitions[0].value else {
            XCTFail("Expected lambda"); return
        }
        XCTAssertEqual(param, "it")
        guard case .apply(let fn, let args, _) = body else {
            XCTFail("Expected apply in body"); return
        }
        guard case .name("do", _) = fn else { XCTFail("Expected 'do'"); return }
        XCTAssertEqual(args.count, 1)
        guard case .name("it", _) = args[0] else { XCTFail("Expected 'it'"); return }
    }

    func testParseLambdaWithPipe() throws {
        let program = try parseSource("f: (it: it | double | increment)")
        XCTAssertEqual(program.definitions.count, 1)
        guard case .lambda(let param, let body, _) = program.definitions[0].value else {
            XCTFail("Expected lambda"); return
        }
        XCTAssertEqual(param, "it")
        guard case .pipe(let clauses, _) = body else {
            XCTFail("Expected pipe in body, got \(body)"); return
        }
        XCTAssertEqual(clauses.count, 3)
        guard case .name("it", _) = clauses[0] else { XCTFail("Expected 'it'"); return }
        guard case .name("double", _) = clauses[1] else { XCTFail("Expected 'double'"); return }
        guard case .name("increment", _) = clauses[2] else { XCTFail("Expected 'increment'"); return }
    }

    func testParseLambdaAsArgument() throws {
        let program = try parseSource("r: (something (it: do it) x)")
        XCTAssertEqual(program.definitions.count, 1)
        guard case .apply(let function, let arguments, _) = program.definitions[0].value else {
            XCTFail("Expected apply"); return
        }
        guard case .name("something", _) = function else {
            XCTFail("Expected 'something'"); return
        }
        XCTAssertEqual(arguments.count, 2)
        guard case .lambda(let param, _, _) = arguments[0] else {
            XCTFail("Expected lambda as first argument, got \(arguments[0])"); return
        }
        XCTAssertEqual(param, "it")
        guard case .name("x", _) = arguments[1] else {
            XCTFail("Expected name 'x' as second argument"); return
        }
    }

    func testParseLambdaNestedInPipe() throws {
        let program = try parseSource("r: (data | (item: transform item))")
        XCTAssertEqual(program.definitions.count, 1)
        guard case .pipe(let clauses, _) = program.definitions[0].value else {
            XCTFail("Expected pipe, got \(program.definitions[0].value)"); return
        }
        XCTAssertEqual(clauses.count, 2)
        guard case .name("data", _) = clauses[0] else {
            XCTFail("Expected 'data'"); return
        }
        guard case .lambda(let param, _, _) = clauses[1] else {
            XCTFail("Expected lambda as pipe clause, got \(clauses[1])"); return
        }
        XCTAssertEqual(param, "item")
    }

    func testParseLambdaDifferentName() throws {
        let program = try parseSource("f: (val: process val)")
        XCTAssertEqual(program.definitions.count, 1)
        guard case .lambda(let param, let body, _) = program.definitions[0].value else {
            XCTFail("Expected lambda"); return
        }
        XCTAssertEqual(param, "val")
        guard case .apply(let fn, let args, _) = body else {
            XCTFail("Expected apply in body"); return
        }
        guard case .name("process", _) = fn else { XCTFail("Expected 'process'"); return }
        XCTAssertEqual(args.count, 1)
        guard case .name("val", _) = args[0] else { XCTFail("Expected 'val'"); return }
    }

    // MARK: - S-Expression Error Cases

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
