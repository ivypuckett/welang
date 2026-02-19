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
}
