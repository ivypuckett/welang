import XCTest
@testable import WeLangLib

final class ParserTests: XCTestCase {

    private func eofToken(pos: Int) -> Token {
        Token(kind: .eof, span: Span(start: pos, end: pos))
    }

    func testParseEofOnly() throws {
        let tokens = [eofToken(pos: 0)]
        let program = try parse(tokens)
        XCTAssertTrue(program.items.isEmpty)
    }

    func testParseEmptySlice() throws {
        let tokens: [Token] = []
        let program = try parse(tokens)
        XCTAssertTrue(program.items.isEmpty)
    }
}
