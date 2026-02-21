import XCTest
@testable import WeLangLib

final class ErrorsTests: XCTestCase {

    // MARK: - Error Display

    func testLexErrorDisplay() {
        let err = LexError.unexpectedCharacter(ch: "@", pos: 5)
        XCTAssertEqual(err.description, "unexpected character '@' at byte 5")
    }

    func testLexErrorInvalidEscapeDisplay() {
        let err = LexError.invalidEscape(ch: "x", pos: 3)
        XCTAssertTrue(err.description.contains("invalid escape"))
        XCTAssertTrue(err.description.contains("x"))
        XCTAssertTrue(err.description.contains("3"))
    }

    func testLexErrorUnterminatedStringDisplay() {
        let err = LexError.unterminatedString(pos: 0)
        XCTAssertTrue(err.description.contains("unterminated string"))
        XCTAssertTrue(err.description.contains("0"))
    }

    func testLexErrorUnterminatedInterpolatedStringDisplay() {
        let err = LexError.unterminatedInterpolatedString(pos: 7)
        XCTAssertTrue(err.description.contains("unterminated interpolated string"))
        XCTAssertTrue(err.description.contains("7"))
    }

    func testLexErrorUnterminatedInterpolationDisplay() {
        let err = LexError.unterminatedInterpolation(pos: 10)
        XCTAssertTrue(err.description.contains("unterminated interpolation"))
        XCTAssertTrue(err.description.contains("10"))
    }

    func testParseErrorDisplay() {
        let err = ParseError.unexpectedToken(span: Span(start: 0, end: 3))
        XCTAssertTrue(err.description.contains("unexpected token"))
    }

    func testParseErrorExpectedClosingParenDisplay() {
        let err = ParseError.expectedClosingParen(span: Span(start: 5, end: 6))
        XCTAssertTrue(err.description.contains("')'"))
    }

    func testParseErrorEmptyClauseDisplay() {
        let err = ParseError.emptyClause(span: Span(start: 3, end: 4))
        XCTAssertTrue(err.description.contains("empty clause"))
    }

    func testCodegenErrorDisplay() {
        let err = CodegenError.llvmError(message: "bad IR")
        XCTAssertEqual(err.description, "LLVM error: bad IR")
    }

    // MARK: - CompileError Wrapping

    func testCompileErrorFromLex() {
        let lexErr = LexError.unexpectedCharacter(ch: "#", pos: 0)
        let compileErr = CompileError.lexer(lexErr)
        if case .lexer = compileErr {
            // OK
        } else {
            XCTFail("Expected .lexer variant, got \(compileErr)")
        }
    }

    func testCompileErrorFromParse() {
        let parseErr = ParseError.unexpectedToken(span: Span(start: 0, end: 1))
        let compileErr = CompileError.parse(parseErr)
        if case .parse = compileErr {
            // OK
        } else {
            XCTFail("Expected .parse variant, got \(compileErr)")
        }
    }

    func testCompileErrorFromCodegen() {
        let cgErr = CodegenError.llvmError(message: "oops")
        let compileErr = CompileError.codegen(cgErr)
        if case .codegen = compileErr {
            // OK
        } else {
            XCTFail("Expected .codegen variant, got \(compileErr)")
        }
    }

    // MARK: - Span

    func testSpanEquality() {
        let a = Span(start: 0, end: 5)
        let b = Span(start: 0, end: 5)
        XCTAssertEqual(a, b)
    }

    func testSpanInequality() {
        let a = Span(start: 0, end: 5)
        let b = Span(start: 1, end: 5)
        XCTAssertNotEqual(a, b)
    }

    func testSpanDescription() {
        let span = Span(start: 3, end: 7)
        XCTAssertTrue(span.description.contains("3"))
        XCTAssertTrue(span.description.contains("7"))
    }

    // MARK: - New Parse Error Display

    func testParseErrorExpectedClosingBraceDisplay() {
        let err = ParseError.expectedClosingBrace(span: Span(start: 5, end: 6))
        XCTAssertTrue(err.description.contains("'}'"))
    }

    func testParseErrorExpectedClosingBracketDisplay() {
        let err = ParseError.expectedClosingBracket(span: Span(start: 5, end: 6))
        XCTAssertTrue(err.description.contains("']'"))
    }

    func testParseErrorExpectedFieldDisplay() {
        let err = ParseError.expectedField(span: Span(start: 2, end: 3))
        XCTAssertTrue(err.description.contains("field"))
        XCTAssertTrue(err.description.contains("'.'"))
    }
}
