import XCTest
@testable import WeLangLib

final class ErrorsTests: XCTestCase {

    // MARK: - Error Display

    func testLexErrorDisplay() {
        let err = LexError.unexpectedCharacter(ch: "@", pos: 5)
        XCTAssertEqual(err.description, "unexpected character '@' at byte 5")
    }

    func testParseErrorDisplay() {
        let err = ParseError.unexpectedToken(span: Span(start: 0, end: 3))
        XCTAssertTrue(err.description.contains("unexpected token"))
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
}
