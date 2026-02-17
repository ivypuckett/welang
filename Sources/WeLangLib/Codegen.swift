import CLLLVM

/// Generate LLVM IR from a parsed `Program` and write the resulting
/// object file (or execute it, depending on future CLI flags).
///
/// Currently this sets up an LLVM module and does nothing with the AST
/// since no language constructs exist yet.
public func generate(_ program: Program) throws {
    let context = LLVMContextCreate()
    defer { LLVMContextDispose(context) }

    let module = LLVMModuleCreateWithNameInContext("welang", context)
    defer { LLVMDisposeModule(module) }

    // Initialize the native target so the JIT can work.
    LLVMLinkInMCJIT()
    LLVM_InitializeNativeTarget()
    LLVM_InitializeNativeAsmPrinter()

    var executionEngine: LLVMExecutionEngineRef?
    var errorMessage: UnsafeMutablePointer<CChar>?

    let result = LLVMCreateJITCompilerForModule(&executionEngine, module, 0, &errorMessage)
    if result != 0 {
        let message: String
        if let errorMessage {
            message = String(cString: errorMessage)
            LLVMDisposeMessage(errorMessage)
        } else {
            message = "unknown LLVM error"
        }
        throw CodegenError.llvmError(message: message)
    }

    defer {
        if let executionEngine {
            LLVMDisposeExecutionEngine(executionEngine)
        }
    }

    // TODO: walk the AST and emit LLVM IR.
}

// MARK: - LLVM Helpers

/// Creates an LLVM context. Caller is responsible for disposal.
public func createLLVMContext() -> LLVMContextRef? {
    return LLVMContextCreate()
}

/// Creates an LLVM module in the given context. Caller is responsible for disposal.
public func createLLVMModule(named name: String, in context: LLVMContextRef?) -> LLVMModuleRef? {
    return LLVMModuleCreateWithNameInContext(name, context)
}

/// Returns the name of an LLVM module as a String.
public func getLLVMModuleName(_ module: LLVMModuleRef?) -> String? {
    guard let module else { return nil }
    var length: Int = 0
    guard let cStr = LLVMGetModuleIdentifier(module, &length) else { return nil }
    return String(cString: cStr)
}

// MARK: - Native Target Initialization Helpers

private func LLVM_InitializeNativeTarget() {
    LLVMInitializeX86TargetInfo()
    LLVMInitializeX86Target()
    LLVMInitializeX86TargetMC()
}

private func LLVM_InitializeNativeAsmPrinter() {
    LLVMInitializeX86AsmPrinter()
}
