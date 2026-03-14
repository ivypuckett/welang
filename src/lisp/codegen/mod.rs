use std::cell::Cell;
use std::str::FromStr;

use cranelift_codegen::ir::AbiParam;
use cranelift_codegen::ir::types;
use cranelift_codegen::settings::{self, Configurable};
use cranelift_module::{DataDescription, DataId, FuncId, Linkage, Module};
use cranelift_object::{ObjectBuilder, ObjectModule};
use target_lexicon::Triple;

use super::parser::Expr;

mod func;
mod ops;

pub(super) type CompileError = String;

pub(super) struct FuncInfo {
    pub id: FuncId,
    pub arity: usize,
    pub is_main: bool,
}

pub(super) struct Builtins {
    pub printf_id: FuncId,
    pub puts_id: FuncId,
    pub malloc_id: FuncId,
    pub fmt_int_id: DataId,
    pub next_str_id: Cell<usize>,
}

use std::collections::HashMap;

fn create_module() -> Result<ObjectModule, CompileError> {
    let mut flag_builder = settings::builder();
    flag_builder
        .set("use_colocated_libcalls", "false")
        .map_err(|e| e.to_string())?;
    flag_builder
        .set("is_pic", "false")
        .map_err(|e| e.to_string())?;
    let flags = settings::Flags::new(flag_builder);
    let triple = Triple::from_str("x86_64-unknown-linux-gnu").map_err(|e| e.to_string())?;
    let isa = cranelift_codegen::isa::lookup(triple)
        .map_err(|e| e.to_string())?
        .finish(flags)
        .map_err(|e| e.to_string())?;
    let obj_builder =
        ObjectBuilder::new(isa, "we_output", cranelift_module::default_libcall_names())
            .map_err(|e| e.to_string())?;
    Ok(ObjectModule::new(obj_builder))
}

pub(super) fn make_sig(
    module: &ObjectModule,
    arity: usize,
    is_main: bool,
) -> cranelift_codegen::ir::Signature {
    let mut sig = module.make_signature();
    if is_main {
        sig.returns.push(AbiParam::new(types::I32));
    } else {
        for _ in 0..arity {
            sig.params.push(AbiParam::new(types::I64));
        }
        sig.returns.push(AbiParam::new(types::I64));
    }
    sig
}

pub fn compile(exprs: &[Expr]) -> Result<Vec<u8>, CompileError> {
    let mut module = create_module()?;

    let printf_id = {
        let mut sig = module.make_signature();
        sig.params.push(AbiParam::new(types::I64));
        sig.params.push(AbiParam::new(types::I64));
        sig.returns.push(AbiParam::new(types::I32));
        module
            .declare_function("printf", Linkage::Import, &sig)
            .map_err(|e| e.to_string())?
    };
    let puts_id = {
        let mut sig = module.make_signature();
        sig.params.push(AbiParam::new(types::I64));
        sig.returns.push(AbiParam::new(types::I32));
        module
            .declare_function("puts", Linkage::Import, &sig)
            .map_err(|e| e.to_string())?
    };
    let malloc_id = {
        let mut sig = module.make_signature();
        sig.params.push(AbiParam::new(types::I64));
        sig.returns.push(AbiParam::new(types::I64));
        module
            .declare_function("malloc", Linkage::Import, &sig)
            .map_err(|e| e.to_string())?
    };
    let fmt_int_id = {
        let mut desc = DataDescription::new();
        desc.define(b"%ld\n\0".to_vec().into_boxed_slice());
        let id = module
            .declare_data("__we_fmt_int", Linkage::Local, false, false)
            .map_err(|e| e.to_string())?;
        module.define_data(id, &desc).map_err(|e| e.to_string())?;
        id
    };

    let builtins = Builtins {
        printf_id,
        puts_id,
        malloc_id,
        fmt_int_id,
        next_str_id: Cell::new(0),
    };

    let mut registry: HashMap<String, FuncInfo> = HashMap::new();

    for expr in exprs {
        if let Expr::List(items) = expr
            && items.len() >= 2
            && let Expr::Symbol(kw) = &items[0]
            && kw == "define"
            && let Expr::List(sig_items) = &items[1]
            && let Some(Expr::Symbol(name)) = sig_items.first()
        {
            let arity = sig_items.len() - 1;
            let is_main = name == "main";
            let sig = make_sig(&module, arity, is_main);
            let id = module
                .declare_function(name, Linkage::Export, &sig)
                .map_err(|e| e.to_string())?;
            registry.insert(name.clone(), FuncInfo { id, arity, is_main });
        }
    }

    if registry.is_empty() {
        return Err("no function definitions found".to_string());
    }

    for expr in exprs {
        if let Expr::List(items) = expr
            && items.len() >= 3
            && let Expr::Symbol(kw) = &items[0]
            && kw == "define"
            && let Expr::List(sig_items) = &items[1]
        {
            func::compile_function(&mut module, &registry, sig_items, &items[2], &builtins)?;
        }
    }

    let product = module.finish();
    product.emit().map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lisp::parser::parse;

    fn compile_src(src: &str) -> Result<Vec<u8>, CompileError> {
        let exprs = parse(src).map_err(|e| format!("{:?}", e))?;
        compile(&exprs)
    }

    #[test]
    fn test_rename_does_not_leak_into_map_sibling() {
        let err = compile_src("f: {a: (n: x), b: n}").unwrap_err();
        assert!(
            err.contains("undefined variable: n"),
            "expected scope-isolation error; got: {err}"
        );
    }

    #[test]
    fn test_rename_does_not_leak_into_cond_sibling() {
        let err = compile_src("f: {(equal [x, 0]): (n: 0), _: n}").unwrap_err();
        assert!(
            err.contains("undefined variable: n"),
            "expected scope-isolation error; got: {err}"
        );
    }

    #[test]
    fn test_rename_body_can_still_use_renamed_var() {
        assert!(
            compile_src("f: (n: (add [n, 1]))").is_ok(),
            "rename body should be able to reference the renamed variable"
        );
    }

    #[test]
    fn test_get_missing_key_is_compile_error() {
        let err = compile_src("main: (get [{x: 1, y: 2}, z])").unwrap_err();
        assert!(
            err.contains("'get' key 'z' not found in map"),
            "unexpected error: {err}"
        );
        assert!(
            err.contains("x, y"),
            "error should list available keys; got: {err}"
        );
    }

    #[test]
    fn test_get_non_literal_map_is_compile_error() {
        let err = compile_src("f: (get [x, key])").unwrap_err();
        assert!(err.contains("map literal"), "unexpected error: {err}");
    }

    #[test]
    fn test_get_empty_map_is_compile_error() {
        let err = compile_src("main: (get [{}, z])").unwrap_err();
        assert!(
            err.contains("'get' key 'z' not found in map"),
            "unexpected error: {err}"
        );
        assert!(
            err.contains("none — map is empty"),
            "error should note the map is empty; got: {err}"
        );
    }
}
