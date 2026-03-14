use std::collections::HashMap;

use cranelift_codegen::ir::types;
use cranelift_codegen::ir::{InstBuilder, Value};
use cranelift_frontend::{FunctionBuilder, FunctionBuilderContext, Variable};
use cranelift_module::Module;
use cranelift_object::ObjectModule;

use super::ops;
use super::{Builtins, CompileError, FuncInfo, make_sig};
use crate::lisp::parser::Expr;

pub(super) fn compile_function(
    module: &mut ObjectModule,
    registry: &HashMap<String, FuncInfo>,
    sig_items: &[Expr],
    body: &Expr,
    builtins: &Builtins,
) -> Result<(), CompileError> {
    let name = match sig_items.first() {
        Some(Expr::Symbol(s)) => s.as_str(),
        _ => return Err("expected function name".to_string()),
    };

    let param_names: Vec<&str> = sig_items[1..]
        .iter()
        .map(|e| match e {
            Expr::Symbol(s) => Ok(s.as_str()),
            _ => Err("function parameters must be symbols".to_string()),
        })
        .collect::<Result<Vec<_>, _>>()?;

    let info = registry
        .get(name)
        .ok_or_else(|| format!("undeclared function: {}", name))?;

    let sig = make_sig(module, param_names.len(), info.is_main);
    let mut ctx = module.make_context();
    ctx.func.signature = sig;

    let mut builder_ctx = FunctionBuilderContext::new();
    {
        let mut builder = FunctionBuilder::new(&mut ctx.func, &mut builder_ctx);
        let entry = builder.create_block();
        builder.append_block_params_for_function_params(entry);
        builder.switch_to_block(entry);
        builder.seal_block(entry);

        let mut locals: HashMap<String, Variable> = HashMap::new();
        let mut next_var: usize = 0;

        if !info.is_main {
            for (i, pname) in param_names.iter().enumerate() {
                let var = Variable::from_u32(next_var as u32);
                next_var += 1;
                builder.declare_var(var, types::I64);
                let pval = builder.block_params(entry)[i];
                builder.def_var(var, pval);
                locals.insert(pname.to_string(), var);
            }
        }

        let result = if let Expr::StructuralType(_) | Expr::NominalType(_) = body {
            if info.is_main {
                builder.ins().iconst(types::I64, 0)
            } else {
                match locals.get("x") {
                    Some(&var) => builder.use_var(var),
                    None => builder.ins().iconst(types::I64, 0),
                }
            }
        } else {
            compile_expr(
                &mut builder,
                module,
                registry,
                body,
                &mut locals,
                &mut next_var,
                builtins,
            )?
        };

        if info.is_main {
            let r32 = builder.ins().ireduce(types::I32, result);
            builder.ins().return_(&[r32]);
        } else {
            builder.ins().return_(&[result]);
        }

        builder.finalize();
    }

    module
        .define_function(info.id, &mut ctx)
        .map_err(|e| e.to_string())?;
    module.clear_context(&mut ctx);
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub(super) fn compile_expr(
    builder: &mut FunctionBuilder,
    module: &mut ObjectModule,
    registry: &HashMap<String, FuncInfo>,
    expr: &Expr,
    locals: &mut HashMap<String, Variable>,
    next_var: &mut usize,
    builtins: &Builtins,
) -> Result<Value, CompileError> {
    match expr {
        Expr::Number(n) => Ok(builder.ins().iconst(types::I64, *n as i64)),
        Expr::Bool(b) => Ok(builder.ins().iconst(types::I64, if *b { 1 } else { 0 })),
        Expr::Symbol(name) => locals
            .get(name.as_str())
            .copied()
            .map(|var| builder.use_var(var))
            .ok_or_else(|| format!("undefined variable: {}", name)),
        Expr::Rename(name, body) => {
            let x_val = locals
                .get("x")
                .copied()
                .ok_or_else(|| {
                    format!(
                        "rename '({}: ...)' can only be used in a one-arg function",
                        name
                    )
                })
                .map(|var| builder.use_var(var))?;
            let new_var = Variable::from_u32(*next_var as u32);
            *next_var += 1;
            builder.declare_var(new_var, types::I64);
            builder.def_var(new_var, x_val);
            let mut inner_locals = locals.clone();
            inner_locals.insert(name.clone(), new_var);
            compile_expr(
                builder,
                module,
                registry,
                body,
                &mut inner_locals,
                next_var,
                builtins,
            )
        }
        Expr::Tuple(_) => Err(
            "tuple literal [...] can only appear as an argument to a built-in operator".to_string(),
        ),
        Expr::Map(entries) => ops::compile_map(
            builder, module, registry, entries, locals, next_var, builtins,
        ),
        Expr::Cond(entries) => ops::compile_cond(
            builder, module, registry, entries, locals, next_var, builtins,
        ),
        Expr::List(items) if items.is_empty() => {
            Err("empty list () is not a valid expression".to_string())
        }
        Expr::List(items) if items.len() == 1 => compile_expr(
            builder, module, registry, &items[0], locals, next_var, builtins,
        ),
        Expr::List(items) => match &items[0] {
            Expr::Symbol(op)
                if matches!(op.as_str(), "add" | "subtract" | "multiply" | "divide") =>
            {
                let (lhs, rhs) = ops::unpack_binary_tuple(op, &items[1..])?;
                ops::compile_arith(
                    builder, module, registry, op, lhs, rhs, locals, next_var, builtins,
                )
            }
            Expr::Symbol(op)
                if matches!(
                    op.as_str(),
                    "equal"
                        | "equals"
                        | "lessThan"
                        | "greaterThan"
                        | "lessThanOrEqual"
                        | "greaterThanOrEqual"
                ) =>
            {
                let canonical = if op == "equals" { "equal" } else { op.as_str() };
                let (lhs, rhs) = ops::unpack_binary_tuple(canonical, &items[1..])?;
                ops::compile_cmp(
                    builder, module, registry, canonical, lhs, rhs, locals, next_var, builtins,
                )
            }
            Expr::Symbol(kw) if kw == "print" => ops::compile_print(
                builder,
                module,
                registry,
                &items[1..],
                locals,
                next_var,
                builtins,
            ),
            Expr::Symbol(kw) if kw == "get" => ops::compile_get(
                builder,
                module,
                registry,
                &items[1..],
                locals,
                next_var,
                builtins,
            ),
            Expr::Symbol(name) => ops::compile_call(
                builder,
                module,
                registry,
                name,
                &items[1..],
                locals,
                next_var,
                builtins,
            ),
            other => Err(format!("cannot call {:?} as a function", other)),
        },
        Expr::StructuralType(_) | Expr::NominalType(_) => Ok(builder.ins().iconst(types::I64, 0)),
        Expr::Str(_) => {
            Err("string literals are only supported as arguments to 'print'".to_string())
        }
    }
}
