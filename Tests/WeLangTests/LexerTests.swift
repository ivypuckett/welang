import XCTest
@testable import WeLangLib

final class LexerTests: XCTestCase {

    func testLexEmptySourceReturnsEof() throws {
        let tokens = try lex("")
        XCTAssertEqual(tokens.count, 1)
        XCTAssertEqual(tokens[0].kind, .eof)
        XCTAssertEqual(tokens[0].span, Span(start: 0, end: 0))
    }

    func testLexReturnsEofAtEnd() throws {
        let tokens = try lex("hello")
        let last = try XCTUnwrap(tokens.last)
        XCTAssertEqual(last.kind, .eof)
    }

    func testLexEofSpanMatchesSourceLength() throws {
        let source = "abc"
        let tokens = try lex(source)
        let eof = try XCTUnwrap(tokens.last)
        XCTAssertEqual(eof.span.start, source.utf8.count)
    }
}
