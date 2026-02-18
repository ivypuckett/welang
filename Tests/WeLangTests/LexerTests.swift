import XCTest
@testable import WeLangLib

final class LexerTests: XCTestCase {

    // MARK: - Helpers

    /// Extract just the token kinds from a token array for easier assertions.
    private func kinds(_ source: String) throws -> [TokenKind] {
        try lex(source).map(\.kind)
    }

    // MARK: - Existing Tests (preserved)

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

    // MARK: - Comment Tests

    func testLexCommentIsSkipped() throws {
        let result = try kinds("# comment\n")
        XCTAssertEqual(result, [.newline, .eof])
    }

    func testLexCommentAtEndOfFile() throws {
        let result = try kinds("# comment")
        XCTAssertEqual(result, [.eof])
    }

    func testLexCommentAfterToken() throws {
        let result = try kinds("foo # comment\n")
        XCTAssertEqual(result, [.label("foo"), .newline, .eof])
    }

    // MARK: - Number Tests

    func testLexUnsignedInteger() throws {
        let result = try kinds("42")
        XCTAssertEqual(result, [.integerLiteral("42"), .eof])
    }

    func testLexZero() throws {
        let result = try kinds("0")
        XCTAssertEqual(result, [.integerLiteral("0"), .eof])
    }

    func testLexNegativeInteger() throws {
        let result = try kinds("-1")
        XCTAssertEqual(result, [.integerLiteral("-1"), .eof])
    }

    func testLexFloatLiteral() throws {
        let result = try kinds("3.14")
        XCTAssertEqual(result, [.floatLiteral("3.14"), .eof])
    }

    func testLexNegativeFloat() throws {
        let result = try kinds("-0.5")
        XCTAssertEqual(result, [.floatLiteral("-0.5"), .eof])
    }

    // MARK: - String Tests

    func testLexSimpleString() throws {
        let result = try kinds("\"hello\"")
        XCTAssertEqual(result, [.stringLiteral("hello"), .eof])
    }

    func testLexStringWithEscapes() throws {
        let result = try kinds("\"a\\nb\"")
        XCTAssertEqual(result, [.stringLiteral("a\nb"), .eof])
    }

    func testLexStringWithEscapedQuote() throws {
        let result = try kinds("\"she said \\\"hi\\\"\"")
        XCTAssertEqual(result, [.stringLiteral("she said \"hi\""), .eof])
    }

    func testLexUnterminatedString() throws {
        XCTAssertThrowsError(try lex("\"oops")) { error in
            guard case LexError.unterminatedString(pos: 0) = error else {
                XCTFail("Expected unterminatedString, got \(error)")
                return
            }
        }
    }

    func testLexInvalidEscape() throws {
        XCTAssertThrowsError(try lex("\"bad\\x\"")) { error in
            guard case LexError.invalidEscape(ch: "x", pos: _) = error else {
                XCTFail("Expected invalidEscape, got \(error)")
                return
            }
        }
    }

    func testLexInterpolatedString() throws {
        let result = try kinds("`hello {{name}}`")
        XCTAssertEqual(result, [.interpolatedStringLiteral("hello {{name}}"), .eof])
    }

    func testLexUnterminatedInterpolatedString() throws {
        XCTAssertThrowsError(try lex("`oops")) { error in
            guard case LexError.unterminatedInterpolatedString(pos: 0) = error else {
                XCTFail("Expected unterminatedInterpolatedString, got \(error)")
                return
            }
        }
    }

    func testLexInvalidEscapeInInterpolatedString() throws {
        XCTAssertThrowsError(try lex("`bad\\x`")) { error in
            guard case LexError.invalidEscape(ch: "x", pos: _) = error else {
                XCTFail("Expected invalidEscape, got \(error)")
                return
            }
        }
    }

    // MARK: - Label and Discard Tests

    func testLexLabel() throws {
        let result = try kinds("foo")
        XCTAssertEqual(result, [.label("foo"), .eof])
    }

    func testLexLabelWithDigits() throws {
        let result = try kinds("x2")
        XCTAssertEqual(result, [.label("x2"), .eof])
    }

    func testLexUnderscoredLabel() throws {
        let result = try kinds("_private")
        XCTAssertEqual(result, [.label("_private"), .eof])
    }

    func testLexDiscard() throws {
        let result = try kinds("_")
        XCTAssertEqual(result, [.discard, .eof])
    }

    // MARK: - Punctuation Tests

    func testLexAllPunctuation() throws {
        let result = try kinds("(){}[]:,.|@*'")
        XCTAssertEqual(result, [
            .leftParen, .rightParen,
            .leftBrace, .rightBrace,
            .leftBracket, .rightBracket,
            .colon, .comma, .dot, .pipe,
            .at, .star, .tick,
            .eof,
        ])
    }

    func testLexPipe() throws {
        let result = try kinds("|")
        XCTAssertEqual(result, [.pipe, .eof])
    }

    // MARK: - Newline Handling Tests

    func testLexNewline() throws {
        let result = try kinds("\n")
        XCTAssertEqual(result, [.newline, .eof])
    }

    func testLexCollapseBlankLines() throws {
        let result = try kinds("\n\n\n")
        XCTAssertEqual(result, [.newline, .eof])
    }

    func testLexNewlineBetweenTokens() throws {
        let result = try kinds("a\nb")
        XCTAssertEqual(result, [.label("a"), .newline, .label("b"), .eof])
    }

    // MARK: - Compound Expression Tests

    func testLexDefinition() throws {
        let result = try kinds("zero: 0")
        XCTAssertEqual(result, [.label("zero"), .colon, .integerLiteral("0"), .eof])
    }

    func testLexSExpression() throws {
        let result = try kinds("(add 1 2)")
        XCTAssertEqual(result, [
            .leftParen, .label("add"), .integerLiteral("1"), .integerLiteral("2"), .rightParen,
            .eof,
        ])
    }

    func testLexPipedExpression() throws {
        let result = try kinds("(1 | 2 | 3)")
        XCTAssertEqual(result, [
            .leftParen,
            .integerLiteral("1"), .pipe,
            .integerLiteral("2"), .pipe,
            .integerLiteral("3"),
            .rightParen,
            .eof,
        ])
    }

    func testLexTupleLiteral() throws {
        let result = try kinds("{1, 0.1}")
        XCTAssertEqual(result, [
            .leftBrace, .integerLiteral("1"), .comma, .floatLiteral("0.1"), .rightBrace,
            .eof,
        ])
    }

    func testLexMacroApplication() throws {
        let result = try kinds("@memoize query")
        XCTAssertEqual(result, [.at, .label("memoize"), .label("query"), .eof])
    }

    func testLexTypeAnnotation() throws {
        let result = try kinds("*u32")
        XCTAssertEqual(result, [.star, .label("u32"), .eof])
    }

    func testLexAliasAnnotation() throws {
        let result = try kinds("'u32")
        XCTAssertEqual(result, [.tick, .label("u32"), .eof])
    }

    // MARK: - Error Tests

    func testLexUnexpectedCharacter() throws {
        XCTAssertThrowsError(try lex("~")) { error in
            guard case LexError.unexpectedCharacter(ch: "~", pos: 0) = error else {
                XCTFail("Expected unexpectedCharacter, got \(error)")
                return
            }
        }
    }

    // MARK: - Edge Cases

    func testLexWhitespaceOnly() throws {
        let result = try kinds("   ")
        XCTAssertEqual(result, [.eof])
    }

    func testLexMultipleDefinitions() throws {
        let source = "x: 1\ny: 2\nz: 3"
        let result = try kinds(source)
        XCTAssertEqual(result, [
            .label("x"), .colon, .integerLiteral("1"), .newline,
            .label("y"), .colon, .integerLiteral("2"), .newline,
            .label("z"), .colon, .integerLiteral("3"),
            .eof,
        ])
    }

    // MARK: - Span Tests

    func testLexLabelSpan() throws {
        let tokens = try lex("foo")
        XCTAssertEqual(tokens[0].span, Span(start: 0, end: 3))
    }

    func testLexIntegerSpan() throws {
        let tokens = try lex("42")
        XCTAssertEqual(tokens[0].span, Span(start: 0, end: 2))
    }

    func testLexStringSpan() throws {
        // "hi" is 4 bytes: " h i "
        let tokens = try lex("\"hi\"")
        XCTAssertEqual(tokens[0].span, Span(start: 0, end: 4))
    }

    func testLexSpanWithLeadingWhitespace() throws {
        let tokens = try lex("  foo")
        XCTAssertEqual(tokens[0].span, Span(start: 2, end: 5))
    }

    // MARK: - Additional String Escape Tests

    func testLexStringAllEscapes() throws {
        let result = try kinds("\"\\\\\\\"\\n\\t\\r\\0\"")
        XCTAssertEqual(result, [.stringLiteral("\\\"\n\t\r\0"), .eof])
    }

    func testLexEmptyString() throws {
        let result = try kinds("\"\"")
        XCTAssertEqual(result, [.stringLiteral(""), .eof])
    }

    // MARK: - Negative Number Edge Cases

    func testLexDashAloneIsError() throws {
        XCTAssertThrowsError(try lex("-")) { error in
            guard case LexError.unexpectedCharacter(ch: "-", pos: 0) = error else {
                XCTFail("Expected unexpectedCharacter for lone dash, got \(error)")
                return
            }
        }
    }

    // MARK: - Interpolated String Escape Tests

    func testLexInterpolatedStringWithValidEscapes() throws {
        // `a\{b\\c\`` should store raw content with escapes preserved
        let result = try kinds("`a\\{b\\\\c\\``")
        XCTAssertEqual(result, [.interpolatedStringLiteral("a\\{b\\\\c\\`"), .eof])
    }

    // MARK: - Comment and Newline Interaction

    func testLexCommentBetweenNewlinesCollapses() throws {
        let result = try kinds("a\n# comment\nb")
        XCTAssertEqual(result, [.label("a"), .newline, .label("b"), .eof])
    }

    func testLexMultipleCommentsCollapse() throws {
        let result = try kinds("a\n# c1\n# c2\nb")
        XCTAssertEqual(result, [.label("a"), .newline, .label("b"), .eof])
    }
}
