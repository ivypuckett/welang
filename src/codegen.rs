use inkwell::OptimizationLevel;
use inkwell::context::Context;

use crate::ast::Program;
use crate::errors::CodegenError;

/// Generate LLVM IR from a parsed [`Program`] and write the resulting
/// object file (or execute it, depending on future CLI flags).
///
/// Currently this sets up an LLVM module and does nothing with the AST
/// since no language constructs exist yet.
pub fn generate(_program: &Program) -> Result<(), CodegenError> {
    let context = Context::create();
    let module = context.create_module("welang");
    let _execution_engine = module
        .create_jit_execution_engine(OptimizationLevel::None)
        .map_err(|e| CodegenError::LlvmError {
            message: e.to_string(),
        })?;

    // TODO: walk the AST and emit LLVM IR.

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::Program;

    #[test]
    fn generate_empty_program() {
        let program = Program { items: vec![] };
        let result = generate(&program);
        assert!(
            result.is_ok(),
            "codegen on empty program failed: {:?}",
            result
        );
    }

    #[test]
    fn llvm_context_creation() {
        // Verify we can create an LLVM context without panicking.
        let _ctx = Context::create();
    }

    #[test]
    fn llvm_module_creation() {
        let ctx = Context::create();
        let module = ctx.create_module("test");
        assert_eq!(module.get_name().to_str().unwrap(), "test");
    }
}
