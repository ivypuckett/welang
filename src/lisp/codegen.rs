use std::collections::HashMap;
use std::str::FromStr;

use cranelift_codegen::ir::condcodes::IntCC;
use cranelift_codegen::ir::types;
use cranelift_codegen::ir::{AbiParam, InstBuilder, Value};
use cranelift_codegen::settings::{self, Configurable};
use cranelift_frontend::{FunctionBuilder, FunctionBuilderContext, Variable};
use cranelift_module::{FuncId, Linkage, Module};
use cranelift_object::{ObjectBuilder, ObjectModule};
use target_lexicon::Triple;

use super::parser::Expr;

type CompileError = String;

struct FuncInfo {
    id: FuncId,
    arity: usize,
    is_main: bool,
}

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

fn make_sig(
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

/// Compile a slice of top-level AST nodes into an ELF object file (raw bytes).
///
/// Top-level forms (produced by the parser):
///   `(define (name) body)`      — zero-arg function
///   `(define (name x) body)`    — one-arg function; input is `x`
///
/// Inside function bodies:
///   number / bool literals
///   `x`                         — the implicit input parameter
///   `(op [a, b])`               — arithmetic: `+  -  *  /`
///   `(op [a, b])`               — comparison: `=  <  >  <=  >=`  (returns 0 or 1)
///   `(if [cond, then])`         — conditional (else = 0)
///   `(if [cond, then, else])`   — conditional with else branch
///   `(name: body)`              — rename `x` to `name` in `body`
///   `(f arg)`                   — call one-arg function
///   `(f)`                       — call zero-arg function
pub fn compile(exprs: &[Expr]) -> Result<Vec<u8>, CompileError> {
    let mut module = create_module()?;

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
            compile_function(&mut module, &registry, sig_items, &items[2..])?;
        }
    }

    let product = module.finish();
    product.emit().map_err(|e| e.to_string())
}

fn compile_function(
    module: &mut ObjectModule,
    registry: &HashMap<String, FuncInfo>,
    sig_items: &[Expr],
    body: &[Expr],
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

        for (i, pname) in param_names.iter().enumerate() {
            let var = Variable::from_u32(next_var as u32);
            next_var += 1;
            builder.declare_var(var, types::I64);
            let pval = builder.block_params(entry)[i];
            builder.def_var(var, pval);
            locals.insert(pname.to_string(), var);
        }

        let mut result = builder.ins().iconst(types::I64, 0);
        for expr in body {
            result = compile_expr(
                &mut builder,
                module,
                registry,
                expr,
                &mut locals,
                &mut next_var,
            )?;
        }

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

fn compile_expr(
    builder: &mut FunctionBuilder,
    module: &mut ObjectModule,
    registry: &HashMap<String, FuncInfo>,
    expr: &Expr,
    locals: &mut HashMap<String, Variable>,
    next_var: &mut usize,
) -> Result<Value, CompileError> {
    match expr {
        Expr::Number(n) => Ok(builder.ins().iconst(types::I64, *n as i64)),

        Expr::Bool(b) => Ok(builder.ins().iconst(types::I64, if *b { 1 } else { 0 })),

        Expr::Symbol(name) => {
            if let Some(&var) = locals.get(name.as_str()) {
                Ok(builder.use_var(var))
            } else {
                Err(format!("undefined variable: {}", name))
            }
        }

        // `(name: body)` — rename `x` to `name` in scope of `body`.
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
            locals.insert(name.clone(), new_var);
            compile_expr(builder, module, registry, body, locals, next_var)
        }

        // Tuple at expression level is only valid as an argument to an operator.
        Expr::Tuple(_) => Err(
            "tuple literal [...] can only appear as an argument to a built-in operator".to_string(),
        ),

        Expr::List(items) if items.is_empty() => {
            Err("empty list () is not a valid expression".to_string())
        }

        // Single-element list `(expr)` is grouping — evaluates the inner expression.
        Expr::List(items) if items.len() == 1 => {
            compile_expr(builder, module, registry, &items[0], locals, next_var)
        }

        Expr::List(items) => match &items[0] {
            Expr::Symbol(op) if matches!(op.as_str(), "+" | "-" | "*" | "/") => {
                let (lhs, rhs) = unpack_binary_tuple(op, &items[1..])?;
                compile_arith(builder, module, registry, op, lhs, rhs, locals, next_var)
            }
            Expr::Symbol(op) if matches!(op.as_str(), "=" | "<" | ">" | "<=" | ">=") => {
                let (lhs, rhs) = unpack_binary_tuple(op, &items[1..])?;
                compile_cmp(builder, module, registry, op, lhs, rhs, locals, next_var)
            }
            Expr::Symbol(kw) if kw == "if" => {
                compile_if(builder, module, registry, &items[1..], locals, next_var)
            }
            Expr::Symbol(name) => compile_call(
                builder,
                module,
                registry,
                name,
                &items[1..],
                locals,
                next_var,
            ),
            other => Err(format!("cannot call {:?} as a function", other)),
        },

        Expr::Quote(_) => Err("quoted expressions are not supported in compiled code".to_string()),
        Expr::Str(_) => Err("string literals are not yet supported in compiled code".to_string()),
    }
}

/// Expect exactly one argument which is a 2-element tuple `[a, b]`.
/// Returns references to the two elements.
fn unpack_binary_tuple<'a>(
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
fn compile_arith(
    builder: &mut FunctionBuilder,
    module: &mut ObjectModule,
    registry: &HashMap<String, FuncInfo>,
    op: &str,
    lhs: &Expr,
    rhs: &Expr,
    locals: &mut HashMap<String, Variable>,
    next_var: &mut usize,
) -> Result<Value, CompileError> {
    let lv = compile_expr(builder, module, registry, lhs, locals, next_var)?;
    let rv = compile_expr(builder, module, registry, rhs, locals, next_var)?;
    Ok(match op {
        "+" => builder.ins().iadd(lv, rv),
        "-" => builder.ins().isub(lv, rv),
        "*" => builder.ins().imul(lv, rv),
        "/" => builder.ins().sdiv(lv, rv),
        _ => unreachable!(),
    })
}

#[allow(clippy::too_many_arguments)]
fn compile_cmp(
    builder: &mut FunctionBuilder,
    module: &mut ObjectModule,
    registry: &HashMap<String, FuncInfo>,
    op: &str,
    lhs: &Expr,
    rhs: &Expr,
    locals: &mut HashMap<String, Variable>,
    next_var: &mut usize,
) -> Result<Value, CompileError> {
    let lv = compile_expr(builder, module, registry, lhs, locals, next_var)?;
    let rv = compile_expr(builder, module, registry, rhs, locals, next_var)?;
    let cc = match op {
        "=" => IntCC::Equal,
        "<" => IntCC::SignedLessThan,
        ">" => IntCC::SignedGreaterThan,
        "<=" => IntCC::SignedLessThanOrEqual,
        ">=" => IntCC::SignedGreaterThanOrEqual,
        _ => unreachable!(),
    };
    let b = builder.ins().icmp(cc, lv, rv);
    Ok(builder.ins().uextend(types::I64, b))
}

fn compile_if(
    builder: &mut FunctionBuilder,
    module: &mut ObjectModule,
    registry: &HashMap<String, FuncInfo>,
    args: &[Expr],
    locals: &mut HashMap<String, Variable>,
    next_var: &mut usize,
) -> Result<Value, CompileError> {
    // Expect a 2-element `[cond, then]` or 3-element `[cond, then, else]` tuple.
    let tuple_elems = match args {
        [Expr::Tuple(elems)] if elems.len() == 2 || elems.len() == 3 => elems,
        _ => {
            return Err(
                "'if' requires a tuple argument: [cond, then] or [cond, then, else]".to_string(),
            );
        }
    };

    let cond = compile_expr(builder, module, registry, &tuple_elems[0], locals, next_var)?;
    let zero = builder.ins().iconst(types::I64, 0);
    let flag = builder.ins().icmp(IntCC::NotEqual, cond, zero);

    let then_block = builder.create_block();
    let else_block = builder.create_block();
    let merge_block = builder.create_block();
    builder.append_block_param(merge_block, types::I64);

    builder.ins().brif(flag, then_block, &[], else_block, &[]);

    builder.switch_to_block(then_block);
    builder.seal_block(then_block);
    let then_val = compile_expr(builder, module, registry, &tuple_elems[1], locals, next_var)?;
    builder.ins().jump(merge_block, &[then_val]);

    builder.switch_to_block(else_block);
    builder.seal_block(else_block);
    let else_val = if tuple_elems.len() == 3 {
        compile_expr(builder, module, registry, &tuple_elems[2], locals, next_var)?
    } else {
        builder.ins().iconst(types::I64, 0)
    };
    builder.ins().jump(merge_block, &[else_val]);

    builder.switch_to_block(merge_block);
    builder.seal_block(merge_block);
    Ok(builder.block_params(merge_block)[0])
}

fn compile_call(
    builder: &mut FunctionBuilder,
    module: &mut ObjectModule,
    registry: &HashMap<String, FuncInfo>,
    name: &str,
    args: &[Expr],
    locals: &mut HashMap<String, Variable>,
    next_var: &mut usize,
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
        .map(|a| compile_expr(builder, module, registry, a, locals, next_var))
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
