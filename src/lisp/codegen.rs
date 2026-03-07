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
        // C `int main(void)` — returns i32
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
/// Supported top-level forms:
/// - `name: (args...) body...` — function definition
/// - `(define name number)`    — global numeric constant
///
/// Inside function bodies:
/// - number / bool literals
/// - symbol references (parameters, locals, global constants)
/// - `(+ - * / a b ...)` — arithmetic
/// - `(= < > <= >= a b)` — comparisons (return 0 or 1)
/// - `(if cond then [else])` — conditional
/// - `(define name expr)`  — local variable binding
/// - `(begin e1 e2 ...)`   — sequence, returns last value
/// - `(f arg ...)`         — function call
pub fn compile(exprs: &[Expr]) -> Result<Vec<u8>, CompileError> {
    let mut module = create_module()?;

    // Collect simple global constants and declare all top-level functions.
    let mut global_consts: HashMap<String, i64> = HashMap::new();
    let mut registry: HashMap<String, FuncInfo> = HashMap::new();

    for expr in exprs {
        if let Expr::List(items) = expr
            && items.len() >= 2
            && let Expr::Symbol(kw) = &items[0]
            && kw == "define"
        {
            match &items[1] {
                Expr::List(sig_items) => {
                    if let Some(Expr::Symbol(name)) = sig_items.first() {
                        let arity = sig_items.len() - 1;
                        let is_main = name == "main";
                        let sig = make_sig(&module, arity, is_main);
                        let id = module
                            .declare_function(name, Linkage::Export, &sig)
                            .map_err(|e| e.to_string())?;
                        registry.insert(name.clone(), FuncInfo { id, arity, is_main });
                    }
                }
                Expr::Symbol(name) => {
                    if let Some(Expr::Number(n)) = items.get(2) {
                        global_consts.insert(name.clone(), *n as i64);
                    }
                }
                _ => {}
            }
        }
    }

    if registry.is_empty() {
        return Err("no function definitions found — define at least one function".to_string());
    }

    // Compile each function body.
    for expr in exprs {
        if let Expr::List(items) = expr
            && items.len() >= 3
            && let Expr::Symbol(kw) = &items[0]
            && kw == "define"
            && let Expr::List(sig_items) = &items[1]
        {
            compile_function(
                &mut module,
                &registry,
                &global_consts,
                sig_items,
                &items[2..],
            )?;
        }
    }

    let product = module.finish();
    product.emit().map_err(|e| e.to_string())
}

fn compile_function(
    module: &mut ObjectModule,
    registry: &HashMap<String, FuncInfo>,
    global_consts: &HashMap<String, i64>,
    sig_items: &[Expr],
    body: &[Expr],
) -> Result<(), CompileError> {
    let name = match sig_items.first() {
        Some(Expr::Symbol(s)) => s.as_str(),
        _ => return Err("expected function name as first element of signature".to_string()),
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

        // Evaluate each body expression; the last one is the return value.
        let mut result = builder.ins().iconst(types::I64, 0);
        for expr in body {
            result = compile_expr(
                &mut builder,
                module,
                registry,
                global_consts,
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
    global_consts: &HashMap<String, i64>,
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
            } else if let Some(&val) = global_consts.get(name.as_str()) {
                Ok(builder.ins().iconst(types::I64, val))
            } else {
                Err(format!("undefined variable: {}", name))
            }
        }

        Expr::List(items) if items.is_empty() => Err("cannot evaluate an empty list".to_string()),

        Expr::List(items) => match &items[0] {
            Expr::Symbol(op) if matches!(op.as_str(), "+" | "-" | "*" | "/") => compile_arith(
                builder,
                module,
                registry,
                global_consts,
                op,
                &items[1..],
                locals,
                next_var,
            ),
            Expr::Symbol(op) if matches!(op.as_str(), "=" | "<" | ">" | "<=" | ">=") => {
                compile_cmp(
                    builder,
                    module,
                    registry,
                    global_consts,
                    op,
                    &items[1..],
                    locals,
                    next_var,
                )
            }
            Expr::Symbol(kw) if kw == "if" => compile_if(
                builder,
                module,
                registry,
                global_consts,
                &items[1..],
                locals,
                next_var,
            ),
            Expr::Symbol(kw) if kw == "define" => compile_local_define(
                builder,
                module,
                registry,
                global_consts,
                &items[1..],
                locals,
                next_var,
            ),
            Expr::Symbol(kw) if kw == "begin" => {
                let mut val = builder.ins().iconst(types::I64, 0);
                for e in &items[1..] {
                    val = compile_expr(
                        builder,
                        module,
                        registry,
                        global_consts,
                        e,
                        locals,
                        next_var,
                    )?;
                }
                Ok(val)
            }
            Expr::Symbol(name) => compile_call(
                builder,
                module,
                registry,
                global_consts,
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

#[allow(clippy::too_many_arguments)]
fn compile_arith(
    builder: &mut FunctionBuilder,
    module: &mut ObjectModule,
    registry: &HashMap<String, FuncInfo>,
    global_consts: &HashMap<String, i64>,
    op: &str,
    args: &[Expr],
    locals: &mut HashMap<String, Variable>,
    next_var: &mut usize,
) -> Result<Value, CompileError> {
    if args.len() < 2 {
        return Err(format!("'{}' requires at least 2 arguments", op));
    }
    let mut acc = compile_expr(
        builder,
        module,
        registry,
        global_consts,
        &args[0],
        locals,
        next_var,
    )?;
    for arg in &args[1..] {
        let rhs = compile_expr(
            builder,
            module,
            registry,
            global_consts,
            arg,
            locals,
            next_var,
        )?;
        acc = match op {
            "+" => builder.ins().iadd(acc, rhs),
            "-" => builder.ins().isub(acc, rhs),
            "*" => builder.ins().imul(acc, rhs),
            "/" => builder.ins().sdiv(acc, rhs),
            _ => unreachable!(),
        };
    }
    Ok(acc)
}

#[allow(clippy::too_many_arguments)]
fn compile_cmp(
    builder: &mut FunctionBuilder,
    module: &mut ObjectModule,
    registry: &HashMap<String, FuncInfo>,
    global_consts: &HashMap<String, i64>,
    op: &str,
    args: &[Expr],
    locals: &mut HashMap<String, Variable>,
    next_var: &mut usize,
) -> Result<Value, CompileError> {
    if args.len() != 2 {
        return Err(format!("'{}' requires exactly 2 arguments", op));
    }
    let lhs = compile_expr(
        builder,
        module,
        registry,
        global_consts,
        &args[0],
        locals,
        next_var,
    )?;
    let rhs = compile_expr(
        builder,
        module,
        registry,
        global_consts,
        &args[1],
        locals,
        next_var,
    )?;
    let cc = match op {
        "=" => IntCC::Equal,
        "<" => IntCC::SignedLessThan,
        ">" => IntCC::SignedGreaterThan,
        "<=" => IntCC::SignedLessThanOrEqual,
        ">=" => IntCC::SignedGreaterThanOrEqual,
        _ => unreachable!(),
    };
    let b = builder.ins().icmp(cc, lhs, rhs);
    // icmp returns i8; extend to i64 so callers always see a uniform type.
    Ok(builder.ins().uextend(types::I64, b))
}

fn compile_if(
    builder: &mut FunctionBuilder,
    module: &mut ObjectModule,
    registry: &HashMap<String, FuncInfo>,
    global_consts: &HashMap<String, i64>,
    args: &[Expr],
    locals: &mut HashMap<String, Variable>,
    next_var: &mut usize,
) -> Result<Value, CompileError> {
    if args.len() < 2 || args.len() > 3 {
        return Err("'if' requires 2 or 3 arguments (condition, then [, else])".to_string());
    }

    let cond = compile_expr(
        builder,
        module,
        registry,
        global_consts,
        &args[0],
        locals,
        next_var,
    )?;
    let zero = builder.ins().iconst(types::I64, 0);
    let flag = builder.ins().icmp(IntCC::NotEqual, cond, zero);

    let then_block = builder.create_block();
    let else_block = builder.create_block();
    let merge_block = builder.create_block();
    builder.append_block_param(merge_block, types::I64);

    builder.ins().brif(flag, then_block, &[], else_block, &[]);

    // then branch
    builder.switch_to_block(then_block);
    builder.seal_block(then_block);
    let then_val = compile_expr(
        builder,
        module,
        registry,
        global_consts,
        &args[1],
        locals,
        next_var,
    )?;
    builder.ins().jump(merge_block, &[then_val]);

    // else branch
    builder.switch_to_block(else_block);
    builder.seal_block(else_block);
    let else_val = if args.len() == 3 {
        compile_expr(
            builder,
            module,
            registry,
            global_consts,
            &args[2],
            locals,
            next_var,
        )?
    } else {
        builder.ins().iconst(types::I64, 0)
    };
    builder.ins().jump(merge_block, &[else_val]);

    // merge
    builder.switch_to_block(merge_block);
    builder.seal_block(merge_block);
    Ok(builder.block_params(merge_block)[0])
}

fn compile_local_define(
    builder: &mut FunctionBuilder,
    module: &mut ObjectModule,
    registry: &HashMap<String, FuncInfo>,
    global_consts: &HashMap<String, i64>,
    args: &[Expr],
    locals: &mut HashMap<String, Variable>,
    next_var: &mut usize,
) -> Result<Value, CompileError> {
    if args.len() != 2 {
        return Err("'define' inside a function requires a name and a value".to_string());
    }
    let name = match &args[0] {
        Expr::Symbol(s) => s.clone(),
        _ => return Err("'define' name must be a symbol".to_string()),
    };
    let val = compile_expr(
        builder,
        module,
        registry,
        global_consts,
        &args[1],
        locals,
        next_var,
    )?;
    let var = Variable::from_u32(*next_var as u32);
    *next_var += 1;
    builder.declare_var(var, types::I64);
    builder.def_var(var, val);
    locals.insert(name, var);
    Ok(val)
}

#[allow(clippy::too_many_arguments)]
fn compile_call(
    builder: &mut FunctionBuilder,
    module: &mut ObjectModule,
    registry: &HashMap<String, FuncInfo>,
    global_consts: &HashMap<String, i64>,
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
        .map(|a| {
            compile_expr(
                builder,
                module,
                registry,
                global_consts,
                a,
                locals,
                next_var,
            )
        })
        .collect::<Result<Vec<_>, _>>()?;

    let func_ref = module.declare_func_in_func(info.id, builder.func);
    let call = builder.ins().call(func_ref, &arg_vals);
    // Copy the result Value out before taking another mutable borrow of builder.
    let first_result = builder.inst_results(call).first().copied();

    match first_result {
        None => Ok(builder.ins().iconst(types::I64, 0)),
        Some(r) if info.is_main => {
            // main returns i32 — sign-extend to i64 for uniform internal type
            Ok(builder.ins().sextend(types::I64, r))
        }
        Some(r) => Ok(r),
    }
}
