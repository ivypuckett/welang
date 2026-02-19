import XCTest
import CLLLVM
@testable import WeLangLib

final class CodegenTests: XCTestCase {

    func testGenerateEmptyProgram() throws {
        let program = Program(definitions: [])
        XCTAssertNoThrow(try generate(program))
    }

    func testLlvmContextCreation() {
        // Verify we can create an LLVM context without crashing.
        let ctx = createLLVMContext()
        XCTAssertNotNil(ctx)
        if let ctx { LLVMContextDispose(ctx) }
    }

    func testLlvmModuleCreation() {
        let ctx = createLLVMContext()
        XCTAssertNotNil(ctx)

        let module = createLLVMModule(named: "test", in: ctx)
        XCTAssertNotNil(module)

        let name = getLLVMModuleName(module)
        XCTAssertEqual(name, "test")

        if let module { LLVMDisposeModule(module) }
        if let ctx { LLVMContextDispose(ctx) }
    }
}
