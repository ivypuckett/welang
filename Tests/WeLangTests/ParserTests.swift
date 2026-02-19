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
}
