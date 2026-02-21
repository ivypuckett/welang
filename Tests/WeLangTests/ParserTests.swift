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

    // MARK: - Application Expressions

    func testParseSimpleApplication() throws {
        // (f x) → application(f, x)
        let program = try parseSource("result: (f x)")
        let def = program.definitions[0]
        guard case .application(let fn, let arg, _) = def.value else {
            XCTFail("Expected application, got \(def.value)"); return
        }
        guard case .name(let fnName, _) = fn else { XCTFail("Expected name for fn"); return }
        guard case .name(let argName, _) = arg else { XCTFail("Expected name for arg"); return }
        XCTAssertEqual(fnName, "f")
        XCTAssertEqual(argName, "x")
    }

    func testParseApplicationIsRightAssociative() throws {
        // (f g x) → application(f, application(g, x))
        let program = try parseSource("result: (f g x)")
        let def = program.definitions[0]
        guard case .application(let f, let rest, _) = def.value else {
            XCTFail("Expected application"); return
        }
        guard case .name("f", _) = f else { XCTFail("Expected f"); return }
        guard case .application(let g, let x, _) = rest else {
            XCTFail("Expected nested application"); return
        }
        guard case .name("g", _) = g else { XCTFail("Expected g"); return }
        guard case .name("x", _) = x else { XCTFail("Expected x"); return }
    }

    func testParseFiveTermApplicationChain() throws {
        // (four three two one zero) → application(four, application(three, application(two, application(one, zero))))
        let program = try parseSource("result: (four three two one zero)")
        let def = program.definitions[0]
        guard case .application(let four, let rest1, _) = def.value else { XCTFail(); return }
        guard case .name("four", _) = four else { XCTFail(); return }
        guard case .application(let three, let rest2, _) = rest1 else { XCTFail(); return }
        guard case .name("three", _) = three else { XCTFail(); return }
        guard case .application(let two, let rest3, _) = rest2 else { XCTFail(); return }
        guard case .name("two", _) = two else { XCTFail(); return }
        guard case .application(let one, let zero, _) = rest3 else { XCTFail(); return }
        guard case .name("one", _) = one else { XCTFail(); return }
        guard case .name("zero", _) = zero else { XCTFail(); return }
    }

    func testParseApplicationWithLiteralArg() throws {
        // (inc 1) → application(inc, 1)
        let program = try parseSource("result: (inc 1)")
        let def = program.definitions[0]
        guard case .application(let fn, let arg, _) = def.value else {
            XCTFail("Expected application"); return
        }
        guard case .name("inc", _) = fn else { XCTFail("Expected inc"); return }
        guard case .integerLiteral("1", _) = arg else { XCTFail("Expected literal 1"); return }
    }

    func testParseApplicationAcrossNewlines() throws {
        // Whitespace independence: (f\nx) = (f x)
        let program = try parseSource("result: (f\nx)")
        let def = program.definitions[0]
        guard case .application(let fn, let arg, _) = def.value else {
            XCTFail("Expected application"); return
        }
        guard case .name("f", _) = fn else { XCTFail("Expected f"); return }
        guard case .name("x", _) = arg else { XCTFail("Expected x"); return }
    }

    func testParseNestedGroups() throws {
        // (f (g x)) → application(f, application(g, x))
        let program = try parseSource("result: (f (g x))")
        let def = program.definitions[0]
        guard case .application(let f, let inner, _) = def.value else {
            XCTFail("Expected application"); return
        }
        guard case .name("f", _) = f else { XCTFail("Expected f"); return }
        guard case .application(let g, let x, _) = inner else {
            XCTFail("Expected nested application"); return
        }
        guard case .name("g", _) = g else { XCTFail("Expected g"); return }
        guard case .name("x", _) = x else { XCTFail("Expected x"); return }
    }

    func testParseGroupedSingleExprIsTransparent() throws {
        // (42) is just 42 — parens are transparent
        let program = try parseSource("result: (42)")
        let def = program.definitions[0]
        guard case .integerLiteral("42", _) = def.value else {
            XCTFail("Expected integerLiteral, got \(def.value)"); return
        }
    }

    // MARK: - Pipe Expressions

    func testParsePipeSimple() throws {
        // (x | f) → pipe(x, f) — x feeds into f
        let program = try parseSource("result: (x | f)")
        let def = program.definitions[0]
        guard case .pipe(let input, let fn, _) = def.value else {
            XCTFail("Expected pipe, got \(def.value)"); return
        }
        guard case .name("x", _) = input else { XCTFail("Expected x"); return }
        guard case .name("f", _) = fn else { XCTFail("Expected f"); return }
    }

    func testParsePipeChainIsLeftAssociative() throws {
        // (a | b | c) → pipe(pipe(a, b), c)
        let program = try parseSource("result: (a | b | c)")
        let def = program.definitions[0]
        guard case .pipe(let lhs, let c, _) = def.value else {
            XCTFail("Expected pipe"); return
        }
        guard case .name("c", _) = c else { XCTFail("Expected c"); return }
        guard case .pipe(let a, let b, _) = lhs else {
            XCTFail("Expected inner pipe"); return
        }
        guard case .name("a", _) = a else { XCTFail("Expected a"); return }
        guard case .name("b", _) = b else { XCTFail("Expected b"); return }
    }

    func testParsePipeWithApplicationOnBothSides() throws {
        // (two one zero | four three) →
        //   pipe(application(two, application(one, zero)), application(four, three))
        let program = try parseSource("result: (two one zero | four three)")
        let def = program.definitions[0]
        guard case .pipe(let lhs, let rhs, _) = def.value else {
            XCTFail("Expected pipe"); return
        }
        // Left side: application(two, application(one, zero))
        guard case .application(let two, let oneZero, _) = lhs else {
            XCTFail("Expected application on left of pipe"); return
        }
        guard case .name("two", _) = two else { XCTFail(); return }
        guard case .application(let one, let zero, _) = oneZero else {
            XCTFail("Expected nested application on left of pipe"); return
        }
        guard case .name("one", _) = one else { XCTFail(); return }
        guard case .name("zero", _) = zero else { XCTFail(); return }
        // Right side: application(four, three)
        guard case .application(let four, let three, _) = rhs else {
            XCTFail("Expected application on right of pipe"); return
        }
        guard case .name("four", _) = four else { XCTFail(); return }
        guard case .name("three", _) = three else { XCTFail(); return }
    }

    func testParsePipeAcrossNewlines() throws {
        // (x\n|\nf) = (x | f) — whitespace independent
        let program = try parseSource("result: (x\n|\nf)")
        let def = program.definitions[0]
        guard case .pipe(let input, let fn, _) = def.value else {
            XCTFail("Expected pipe"); return
        }
        guard case .name("x", _) = input else { XCTFail("Expected x"); return }
        guard case .name("f", _) = fn else { XCTFail("Expected f"); return }
    }

    // MARK: - Group Error Cases

    func testParseUnclosedGroupError() throws {
        XCTAssertThrowsError(try parseSource("result: (f x")) { error in
            guard case ParseError.unexpectedToken(_) = error else {
                XCTFail("Expected unexpectedToken, got \(error)"); return
            }
        }
    }

    func testParseEmptyPipeError() throws {
        // (| f) — nothing before the pipe
        XCTAssertThrowsError(try parseSource("result: (| f)")) { error in
            guard case ParseError.expectedExpression(_) = error else {
                XCTFail("Expected expectedExpression, got \(error)"); return
            }
        }
    }

    func testParseTrailingPipeError() throws {
        // (f |) — nothing after the pipe
        XCTAssertThrowsError(try parseSource("result: (f |)")) { error in
            guard case ParseError.expectedExpression(_) = error else {
                XCTFail("Expected expectedExpression, got \(error)"); return
            }
        }
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
}
