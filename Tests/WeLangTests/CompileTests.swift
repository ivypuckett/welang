import XCTest
@testable import WeLangLib

final class CompileTests: XCTestCase {

    func testCompileEmptySource() {
        XCTAssertNoThrow(try compile(""))
    }
}
