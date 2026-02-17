import XCTest
@testable import WeLangLib

final class ASTTests: XCTestCase {

    func testEmptyProgram() {
        let program = Program(items: [])
        XCTAssertTrue(program.items.isEmpty)
    }

    func testProgramEquality() {
        let a = Program(items: [])
        let b = Program(items: [])
        XCTAssertEqual(a, b)
    }

    func testProgramInequality() {
        let a = Program(items: [])
        let b = Program(items: [.placeholder(Span(start: 0, end: 1))])
        XCTAssertNotEqual(a, b)
    }

    func testItemPlaceholderEquality() {
        let a = Item.placeholder(Span(start: 0, end: 1))
        let b = Item.placeholder(Span(start: 0, end: 1))
        XCTAssertEqual(a, b)
    }

    func testItemPlaceholderInequality() {
        let a = Item.placeholder(Span(start: 0, end: 1))
        let b = Item.placeholder(Span(start: 2, end: 3))
        XCTAssertNotEqual(a, b)
    }
}
