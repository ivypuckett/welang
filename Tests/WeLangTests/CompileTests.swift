import XCTest
@testable import WeLangLib

final class CompileTests: XCTestCase {

    func testCompileEmptySource() {
        XCTAssertNoThrow(try compile(""))
    }

    func testCompileSimpleDefinition() {
        XCTAssertNoThrow(try compile("x: 42"))
    }

    func testCompileMultipleDefinitions() {
        XCTAssertNoThrow(try compile("a: 1\nb: 2"))
    }

    func testCompileSExpr() {
        XCTAssertNoThrow(try compile("r: (add 1 2)"))
    }

    func testCompilePipeExpr() {
        XCTAssertNoThrow(try compile("r: (1 | 2 | 3)"))
    }

    func testCompileLambdaExpr() {
        XCTAssertNoThrow(try compile("f: (it: it)"))
    }

    func testCompileNestedExpr() {
        XCTAssertNoThrow(try compile("r: (add (multiply 2 3) 4)"))
    }

    func testCompileLeadingPipe() {
        XCTAssertNoThrow(try compile("f: (| increment)"))
    }
}
