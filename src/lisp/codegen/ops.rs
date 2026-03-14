use std::collections::HashMap;

use cranelift_codegen::ir::condcodes::IntCC;
use cranelift_codegen::ir::types;
use cranelift_codegen::ir::{InstBuilder, MemFlags, Value};
use cranelift_frontend::{FunctionBuilder, Variable};
use cranelift_module::{DataDescription, Linkage, Module};
use cranelift_object::ObjectModule;

use super::func::compile_expr;
use super::{Builtins, CompileError, FuncInfo};
use crate::lisp::parser::Expr;

pub(super) fn unpack_binary_tuple<'a>(
    op: &str,
    args: &'a [Expr],
) -> Result<(&'a Expr, &'a Expr), CompileError> {
    match args {
        [Expr::Tuple(elems)] if elems.len() == 2 => Ok((&elems[0], &elems[1])),
        _ => Err(format!(
            "'{}' requires a 2-element tuple argument [a, b]",
            op
        )),
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn compile_arith(
    builder: &mut FunctionBuilder,
    module: &mut ObjectModule,
    registry: &HashMap<String, FuncInfo>,
    op: &str,
    lhs: &Expr,
    rhs: &Expr,
    locals: &mut HashMap<String, Variable>,
    next_var: &mut usize,
    builtins: &Builtins,
) -> Result<Value, CompileError> {
    let lv = compile_expr(builder, module, registry, lhs, locals, next_var, builtins)?;
    let rv = compile_expr(builder, module, registry, rhs, locals, next_var, builtins)?;
    Ok(match op {
        "add" => builder.ins().iadd(lv, rv),
        "subtract" => builder.ins().isub(lv, rv),
        "multiply" => builder.ins().imul(lv, rv),
        "divide" => builder.ins().sdiv(lv, rv),
        _ => unreachable!(),
    })
}

#[allow(clippy::too_many_arguments)]
pub(super) fn compile_cmp(
    builder: &mut FunctionBuilder,
    module: &mut ObjectModule,
    registry: &HashMap<String, FuncInfo>,
    op: &str,
    lhs: &Expr,
    rhs: &Expr,
    locals: &mut HashMap<String, Variable>,
    next_var: &mut usize,
    builtins: &Builtins,
) -> Result<Value, CompileError> {
    let lv = compile_expr(builder, module, registry, lhs, locals, next_var, builtins)?;
    let rv = compile_expr(builder, module, registry, rhs, locals, next_var, builtins)?;
    let cc = match op {
        "equal" => IntCC::Equal,
        "lessThan" => IntCC::SignedLessThan,
        "greaterThan" => IntCC::SignedGreaterThan,
        "lessThanOrEqual" => IntCC::SignedLessThanOrEqual,
        "greaterThanOrEqual" => IntCC::SignedGreaterThanOrEqual,
        _ => unreachable!(),
    };
    let b = builder.ins().icmp(cc, lv, rv);
    Ok(builder.ins().uextend(types::I64, b))
}

#[allow(clippy::too_many_arguments)]
pub(super) fn compile_cond(
    builder: &mut FunctionBuilder,
    module: &mut ObjectModule,
    registry: &HashMap<String, FuncInfo>,
    entries: &[(Option<Expr>, Expr)],
    locals: &mut HashMap<String, Variable>,
    next_var: &mut usize,
    builtins: &Builtins,
) -> Result<Value, CompileError> {
    let merge_block = builder.create_block();
    builder.append_block_param(merge_block, types::I64);

    for (cond_opt, val_expr) in entries {
        match cond_opt {
            Some(cond) => {
                let cond_val =
                    compile_expr(builder, module, registry, cond, locals, next_var, builtins)?;
                let zero = builder.ins().iconst(types::I64, 0);
                let flag = builder.ins().icmp(IntCC::NotEqual, cond_val, zero);
                let val_block = builder.create_block();
                let next_block = builder.create_block();
                builder.ins().brif(flag, val_block, &[], next_block, &[]);
                builder.switch_to_block(val_block);
                builder.seal_block(val_block);
                let val = compile_expr(
                    builder, module, registry, val_expr, locals, next_var, builtins,
                )?;
                builder.ins().jump(merge_block, &[val]);
                builder.switch_to_block(next_block);
                builder.seal_block(next_block);
            }
            None => {
                let val = compile_expr(
                    builder, module, registry, val_expr, locals, next_var, builtins,
                )?;
                builder.ins().jump(merge_block, &[val]);
            }
        }
    }

    builder.switch_to_block(merge_block);
    builder.seal_block(merge_block);
    Ok(builder.block_params(merge_block)[0])
}

#[allow(clippy::too_many_arguments)]
pub(super) fn compile_print(
    builder: &mut FunctionBuilder,
    module: &mut ObjectModule,
    registry: &HashMap<String, FuncInfo>,
    args: &[Expr],
    locals: &mut HashMap<String, Variable>,
    next_var: &mut usize,
    builtins: &Builtins,
) -> Result<Value, CompileError> {
    if args.len() != 1 {
        return Err(format!("'print' expects 1 argument, got {}", args.len()));
    }

    if let Expr::Str(s) = &args[0] {
        let str_ptr = intern_string(module, builder, builtins, s, "__we_str_")?;
        let puts_ref = module.declare_func_in_func(builtins.puts_id, builder.func);
        builder.ins().call(puts_ref, &[str_ptr]);
        return Ok(builder.ins().iconst(types::I64, 0));
    }

    let val = compile_expr(
        builder, module, registry, &args[0], locals, next_var, builtins,
    )?;
    let gv = module.declare_data_in_func(builtins.fmt_int_id, builder.func);
    let fmt_ptr = builder.ins().global_value(types::I64, gv);
    let printf_ref = module.declare_func_in_func(builtins.printf_id, builder.func);
    builder.ins().call(printf_ref, &[fmt_ptr, val]);
    Ok(val)
}

#[allow(clippy::too_many_arguments)]
pub(super) fn compile_call(
    builder: &mut FunctionBuilder,
    module: &mut ObjectModule,
    registry: &HashMap<String, FuncInfo>,
    name: &str,
    args: &[Expr],
    locals: &mut HashMap<String, Variable>,
    next_var: &mut usize,
    builtins: &Builtins,
) -> Result<Value, CompileError> {
    let info = registry
        .get(name)
        .ok_or_else(|| format!("undefined function: {}", name))?;

    if args.len() != info.arity {
        return Err(format!(
            "function '{}' expects {} argument(s), got {}",
            name,
            info.arity,
            args.len()
        ));
    }

    let arg_vals: Vec<Value> = args
        .iter()
        .map(|a| compile_expr(builder, module, registry, a, locals, next_var, builtins))
        .collect::<Result<Vec<_>, _>>()?;

    let func_ref = module.declare_func_in_func(info.id, builder.func);
    let call = builder.ins().call(func_ref, &arg_vals);
    let first_result = builder.inst_results(call).first().copied();

    match first_result {
        None => Ok(builder.ins().iconst(types::I64, 0)),
        Some(r) if info.is_main => Ok(builder.ins().sextend(types::I64, r)),
        Some(r) => Ok(r),
    }
}

pub(super) fn intern_string(
    module: &mut ObjectModule,
    builder: &mut FunctionBuilder,
    builtins: &Builtins,
    s: &str,
    prefix: &str,
) -> Result<Value, CompileError> {
    let label = format!("{}{}", prefix, builtins.next_str_id.get());
    builtins.next_str_id.set(builtins.next_str_id.get() + 1);
    let mut desc = DataDescription::new();
    let mut bytes = s.as_bytes().to_vec();
    bytes.push(b'\0');
    desc.define(bytes.into_boxed_slice());
    let data_id = module
        .declare_data(&label, Linkage::Local, false, false)
        .map_err(|e| e.to_string())?;
    module
        .define_data(data_id, &desc)
        .map_err(|e| e.to_string())?;
    let gv = module.declare_data_in_func(data_id, builder.func);
    Ok(builder.ins().global_value(types::I64, gv))
}

#[allow(clippy::too_many_arguments)]
pub(super) fn compile_map(
    builder: &mut FunctionBuilder,
    module: &mut ObjectModule,
    registry: &HashMap<String, FuncInfo>,
    entries: &[(String, Expr)],
    locals: &mut HashMap<String, Variable>,
    next_var: &mut usize,
    builtins: &Builtins,
) -> Result<Value, CompileError> {
    let n = entries.len();
    let size = i64::try_from((1 + 2 * n) * 8).map_err(|_| "map literal has too many entries")?;

    let size_val = builder.ins().iconst(types::I64, size);
    let malloc_ref = module.declare_func_in_func(builtins.malloc_id, builder.func);
    let malloc_call = builder.ins().call(malloc_ref, &[size_val]);
    let map_ptr = builder.inst_results(malloc_call)[0];

    let n_val = builder.ins().iconst(
        types::I64,
        i64::try_from(n).map_err(|_| "map literal has too many entries")?,
    );
    builder.ins().store(MemFlags::new(), n_val, map_ptr, 0);

    for (i, (key, val_expr)) in entries.iter().enumerate() {
        let key_ptr = intern_string(module, builder, builtins, key, "__we_mk_")?;
        let key_off =
            i32::try_from((1 + 2 * i) * 8).map_err(|_| "map literal has too many entries")?;
        builder
            .ins()
            .store(MemFlags::new(), key_ptr, map_ptr, key_off);

        let val = if let Expr::Str(s) = val_expr {
            intern_string(module, builder, builtins, s, "__we_str_")?
        } else {
            compile_expr(
                builder, module, registry, val_expr, locals, next_var, builtins,
            )?
        };
        let val_off =
            i32::try_from((2 + 2 * i) * 8).map_err(|_| "map literal has too many entries")?;
        builder.ins().store(MemFlags::new(), val, map_ptr, val_off);
    }

    Ok(map_ptr)
}

#[allow(clippy::too_many_arguments)]
pub(super) fn compile_get(
    builder: &mut FunctionBuilder,
    module: &mut ObjectModule,
    registry: &HashMap<String, FuncInfo>,
    args: &[Expr],
    locals: &mut HashMap<String, Variable>,
    next_var: &mut usize,
    builtins: &Builtins,
) -> Result<Value, CompileError> {
    let (map_expr, key_expr) = match args {
        [Expr::Tuple(elems)] if elems.len() == 2 => (&elems[0], &elems[1]),
        _ => return Err("'get' requires a 2-element tuple argument [map, key]".to_string()),
    };

    let key_name: String = match key_expr {
        Expr::Symbol(s) => s.clone(),
        Expr::Str(s) => s.clone(),
        _ => return Err("'get' key must be a symbol or string literal".to_string()),
    };

    let keys: Vec<&str> =
        match map_expr {
            Expr::Map(entries) => entries.iter().map(|(k, _)| k.as_str()).collect(),
            _ => return Err(
                "'get' map argument must be a map literal so its keys are known at compile time"
                    .to_string(),
            ),
        };

    let key_index = keys.iter().position(|&k| k == key_name).ok_or_else(|| {
        let available = if keys.is_empty() {
            "(none — map is empty)".to_string()
        } else {
            keys.join(", ")
        };
        format!(
            "'get' key '{}' not found in map; available keys: {}",
            key_name, available
        )
    })?;

    let map_ptr = compile_expr(
        builder, module, registry, map_expr, locals, next_var, builtins,
    )?;

    let val_offset =
        i32::try_from((2 + 2 * key_index) * 8).map_err(|_| "'get': key index overflow")?;
    Ok(builder
        .ins()
        .load(types::I64, MemFlags::new(), map_ptr, val_offset))
}
