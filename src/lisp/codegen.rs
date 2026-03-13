use std::cell::Cell;
use std::collections::HashMap;
use std::str::FromStr;

use cranelift_codegen::ir::condcodes::IntCC;
use cranelift_codegen::ir::types;
use cranelift_codegen::ir::{AbiParam, InstBuilder, MemFlags, Value};
use cranelift_codegen::settings::{self, Configurable};
use cranelift_frontend::{FunctionBuilder, FunctionBuilderContext, Variable};
use cranelift_module::{DataDescription, DataId, FuncId, Linkage, Module};
use cranelift_object::{ObjectBuilder, ObjectModule};
use target_lexicon::Triple;

use super::parser::Expr;

type CompileError = String;

struct FuncInfo {
    id: FuncId,
    arity: usize,
    is_main: bool,
}

/// Pre-declared external functions and data used by built-in operations.
struct Builtins {
    /// `printf(const char*, i64) -> i32` — used by `(print n)` for integers.
    printf_id: FuncId,
    /// `puts(const char*) -> i32` — used by `(print "s")` for string literals.
    puts_id: FuncId,
    /// `malloc(size: i64) -> i64` — used by map literals to allocate storage.
    malloc_id: FuncId,
    /// `"%ld\n"` format string for integer printing.
    fmt_int_id: DataId,
    /// Counter for generating unique names for string-literal data objects.
    next_str_id: Cell<usize>,
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
///   `(define (name x) body)`    — function with implicit parameter `x`
///
/// If `body` does not reference `x`, the function behaves as a
/// zero-argument function. `main` is always compiled with no ABI parameters.
///
/// Inside function bodies:
///   number / bool literals
///   `x`                         — the implicit input parameter
///   `(op [a, b])`               — arithmetic: `add  subtract  multiply  divide`
///   `(op [a, b])`               — comparison: `equal  lessThan  greaterThan  lessThanOrEqual  greaterThanOrEqual`  (returns 0 or 1)
///   `{(c1): v1, ..., _: v}`    — conditional: first truthy arm wins
///   `(name: body)`              — rename `x` to `name` in `body`
///   `(print x)`                 — print integer x as "%ld\n", returns x
///   `(print "s")`               — print string literal s followed by newline, returns 0
///   `(f arg)`                   — call one-arg function
///   `(f)`                       — call zero-arg function
pub fn compile(exprs: &[Expr]) -> Result<Vec<u8>, CompileError> {
    let mut module = create_module()?;

    // Declare printf(const char*, i64) -> i32 for integer printing.
    // We treat it as non-variadic here because we always pass exactly one i64;
    // this matches the x86_64 System V ABI for this specific call pattern.
    let printf_id = {
        let mut sig = module.make_signature();
        sig.params.push(AbiParam::new(types::I64)); // format string pointer
        sig.params.push(AbiParam::new(types::I64)); // integer value
        sig.returns.push(AbiParam::new(types::I32));
        module
            .declare_function("printf", Linkage::Import, &sig)
            .map_err(|e| e.to_string())?
    };

    // Declare puts(const char*) -> i32 for string literal printing.
    let puts_id = {
        let mut sig = module.make_signature();
        sig.params.push(AbiParam::new(types::I64)); // string pointer
        sig.returns.push(AbiParam::new(types::I32));
        module
            .declare_function("puts", Linkage::Import, &sig)
            .map_err(|e| e.to_string())?
    };

    // Declare malloc(size: i64) -> i64 for map heap allocation.
    let malloc_id = {
        let mut sig = module.make_signature();
        sig.params.push(AbiParam::new(types::I64)); // byte count
        sig.returns.push(AbiParam::new(types::I64)); // pointer
        module
            .declare_function("malloc", Linkage::Import, &sig)
            .map_err(|e| e.to_string())?
    };

    // Define the "%ld\n" format string used by (print n).
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
            // items[2] is always the body; items[3] (if present) is the
            // type annotation, which is compile-time only — skip it here.
            compile_function(&mut module, &registry, sig_items, &items[2], &builtins)?;
        }
    }

    let product = module.finish();
    product.emit().map_err(|e| e.to_string())
}

fn compile_function(
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

        // `main` has no ABI parameters (the OS entry point convention),
        // so skip binding `x` even though the parser always emits it.
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

        // Structural-type and nominal-type functions are identity functions:
        // they return their argument `x` unchanged (the type check is
        // compile-time only).
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
fn compile_expr(
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

        Expr::Symbol(name) => {
            if let Some(&var) = locals.get(name.as_str()) {
                Ok(builder.use_var(var))
            } else if let Some(info) = registry.get(name.as_str()) {
                // First-class function: resolve to a function pointer (i64).
                let func_ref = module.declare_func_in_func(info.id, builder.func);
                Ok(builder.ins().func_addr(types::I64, func_ref))
            } else {
                Err(format!("undefined variable: {}", name))
            }
        }

        // `(name: body)` — rename `x` to `name` in scope of `body`.
        //
        // We clone `locals` so that the new binding is visible inside `body`
        // but does NOT leak out to sibling expressions compiled afterward
        // (e.g. subsequent map-literal values or later conditional arms).
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

        // Tuple at expression level is only valid as an argument to an operator.
        Expr::Tuple(_) => Err(
            "tuple literal [...] can only appear as an argument to a built-in operator".to_string(),
        ),

        Expr::Map(entries) => compile_map(
            builder, module, registry, entries, locals, next_var, builtins,
        ),

        Expr::Cond(entries) => compile_cond(
            builder, module, registry, entries, locals, next_var, builtins,
        ),

        Expr::List(items) if items.is_empty() => {
            Err("empty list () is not a valid expression".to_string())
        }

        // Single-element list `(expr)` is grouping — evaluates the inner expression.
        Expr::List(items) if items.len() == 1 => compile_expr(
            builder, module, registry, &items[0], locals, next_var, builtins,
        ),

        Expr::List(items) => match &items[0] {
            Expr::Symbol(op)
                if matches!(op.as_str(), "add" | "subtract" | "multiply" | "divide") =>
            {
                let (lhs, rhs) = unpack_binary_tuple(op, &items[1..])?;
                compile_arith(
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
                let (lhs, rhs) = unpack_binary_tuple(canonical, &items[1..])?;
                compile_cmp(
                    builder, module, registry, canonical, lhs, rhs, locals, next_var, builtins,
                )
            }
            Expr::Symbol(kw) if kw == "print" => compile_print(
                builder,
                module,
                registry,
                &items[1..],
                locals,
                next_var,
                builtins,
            ),
            Expr::Symbol(kw) if kw == "get" => compile_get(
                builder,
                module,
                registry,
                &items[1..],
                locals,
                next_var,
                builtins,
            ),
            Expr::Symbol(name) => compile_call(
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

        // Structural and nominal type expressions in value position compile to
        // the integer 0 — they are compile-time type descriptors with no
        // runtime payload.
        Expr::StructuralType(_) | Expr::NominalType(_) => Ok(builder.ins().iconst(types::I64, 0)),

        Expr::Str(_) => {
            Err("string literals are only supported as arguments to 'print'".to_string())
        }
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
fn compile_cmp(
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

/// Compile a conditional expression `{(cond1): v1, (cond2): v2, _: default}`.
///
/// Conditions are evaluated left to right; the value of the first arm whose
/// condition is non-zero is returned.  The `_` wildcard (represented as
/// `None`) is always the last entry and acts as the default.
#[allow(clippy::too_many_arguments)]
fn compile_cond(
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
                // Wildcard arm — always taken (must be last).
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

/// Compile `(print arg)`.
///
/// - `(print n)` where `n` is an integer expression: prints `"%ld\n"` via
///   `printf`, returns `n`.
/// - `(print "s")` where `"s"` is a string literal: prints the string
///   followed by a newline via `puts`, returns `0`.
#[allow(clippy::too_many_arguments)]
fn compile_print(
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

    // Special-case string literals: store in data section and call puts.
    if let Expr::Str(s) = &args[0] {
        let label = format!("__we_str_{}", builtins.next_str_id.get());
        builtins.next_str_id.set(builtins.next_str_id.get() + 1);

        let mut desc = DataDescription::new();
        let mut bytes = s.as_bytes().to_vec();
        bytes.push(b'\0');
        desc.define(bytes.into_boxed_slice());

        let str_id = module
            .declare_data(&label, Linkage::Local, false, false)
            .map_err(|e| e.to_string())?;
        module
            .define_data(str_id, &desc)
            .map_err(|e| e.to_string())?;

        let gv = module.declare_data_in_func(str_id, builder.func);
        let str_ptr = builder.ins().global_value(types::I64, gv);

        let puts_ref = module.declare_func_in_func(builtins.puts_id, builder.func);
        builder.ins().call(puts_ref, &[str_ptr]);

        return Ok(builder.ins().iconst(types::I64, 0));
    }

    // General case: compile the expression and print it as an integer.
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
fn compile_call(
    builder: &mut FunctionBuilder,
    module: &mut ObjectModule,
    registry: &HashMap<String, FuncInfo>,
    name: &str,
    args: &[Expr],
    locals: &mut HashMap<String, Variable>,
    next_var: &mut usize,
    builtins: &Builtins,
) -> Result<Value, CompileError> {
    // Direct call: the name refers to a known top-level function.
    if let Some(info) = registry.get(name) {
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

        return match first_result {
            None => Ok(builder.ins().iconst(types::I64, 0)),
            Some(r) if info.is_main => Ok(builder.ins().sextend(types::I64, r)),
            Some(r) => Ok(r),
        };
    }

    // Indirect call: the name is a local variable holding a function pointer.
    // All user-defined functions take exactly one i64 argument and return i64.
    if let Some(&var) = locals.get(name) {
        if args.len() != 1 {
            return Err(format!(
                "indirect call via '{}' requires exactly 1 argument, got {}",
                name,
                args.len()
            ));
        }

        let func_ptr = builder.use_var(var);
        let arg_val = compile_expr(
            builder, module, registry, &args[0], locals, next_var, builtins,
        )?;

        let mut sig = module.make_signature();
        sig.params.push(AbiParam::new(types::I64));
        sig.returns.push(AbiParam::new(types::I64));
        let sig_ref = builder.import_signature(sig);

        let call = builder.ins().call_indirect(sig_ref, func_ptr, &[arg_val]);
        return Ok(builder.inst_results(call)[0]);
    }

    Err(format!("undefined function: {}", name))
}

/// Helper: intern a string in the object file's data section and return a
/// global-value pointer to it.  The returned `Value` is an `i64` pointer.
fn intern_string(
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

/// Compile a map literal `{k1: v1, k2: v2, ...}`.
///
/// Layout of the heap block (all fields are i64, 8 bytes each):
///   [0]          n_fields
///   [1 + 2*i]    pointer to null-terminated key string for field i
///   [2 + 2*i]    value for field i
///
/// String values are stored as pointers into the data section.
/// All other values are compiled to i64.
/// Returns an i64 pointer to the allocated block.
#[allow(clippy::too_many_arguments)]
fn compile_map(
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

    // Allocate heap storage via malloc.
    let size_val = builder.ins().iconst(types::I64, size);
    let malloc_ref = module.declare_func_in_func(builtins.malloc_id, builder.func);
    let malloc_call = builder.ins().call(malloc_ref, &[size_val]);
    let map_ptr = builder.inst_results(malloc_call)[0];

    // Store n_fields at offset 0.
    let n_val = builder.ins().iconst(
        types::I64,
        i64::try_from(n).map_err(|_| "map literal has too many entries")?,
    );
    builder.ins().store(MemFlags::new(), n_val, map_ptr, 0);

    for (i, (key, val_expr)) in entries.iter().enumerate() {
        // Store key string pointer.
        let key_ptr = intern_string(module, builder, builtins, key, "__we_mk_")?;
        let key_off =
            i32::try_from((1 + 2 * i) * 8).map_err(|_| "map literal has too many entries")?;
        builder
            .ins()
            .store(MemFlags::new(), key_ptr, map_ptr, key_off);

        // Compile value — string literals become data-section pointers.
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

/// Compile `(get [map, key])`.
///
/// `map` must be a map literal whose keys are known at compile time.
/// `key` must be a symbol or string literal naming an existing field.
///
/// The key's position in the literal is resolved at compile time, and a
/// single direct load is emitted at the statically-known offset — no runtime
/// scan or `strcmp` needed.
#[allow(clippy::too_many_arguments)]
fn compile_get(
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

    // Resolve the map's keys at compile time — only map literals are accepted.
    let keys: Vec<&str> =
        match map_expr {
            Expr::Map(entries) => entries.iter().map(|(k, _)| k.as_str()).collect(),
            _ => return Err(
                "'get' map argument must be a map literal so its keys are known at compile time"
                    .to_string(),
            ),
        };

    // Validate that the requested key exists.
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

    // Compile the map pointer.
    let map_ptr = compile_expr(
        builder, module, registry, map_expr, locals, next_var, builtins,
    )?;

    // Emit a direct load at the statically-known value offset: (2 + 2*index) * 8.
    // Map layout: [n_fields, key0, val0, key1, val1, ...]  (each slot is 8 bytes)
    let val_offset =
        i32::try_from((2 + 2 * key_index) * 8).map_err(|_| "'get': key index overflow")?;
    Ok(builder
        .ins()
        .load(types::I64, MemFlags::new(), map_ptr, val_offset))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lisp::parser::parse;

    fn compile_src(src: &str) -> Result<Vec<u8>, CompileError> {
        let exprs = parse(src).map_err(|e| format!("{:?}", e))?;
        compile(&exprs)
    }

    // ---- rename scope isolation --------------------------------------------------

    #[test]
    fn test_rename_does_not_leak_into_map_sibling() {
        // Before the fix, `n` introduced by `(n: x)` in the first map entry
        // would persist in `locals` and silently resolve in `b: n`.
        // After the fix, `n` is scoped to its body only, so `n` in `b: n`
        // must produce an "undefined variable" compile error.
        let err = compile_src("f: {a: (n: x), b: n}").unwrap_err();
        assert!(
            err.contains("undefined variable: n"),
            "expected scope-isolation error; got: {err}"
        );
    }

    #[test]
    fn test_rename_does_not_leak_into_cond_sibling() {
        // Before the fix, `n` introduced in the first conditional arm would
        // persist in `locals` and silently resolve in the wildcard arm.
        // After the fix, `n` is scoped to its arm only, so `n` in `_: n`
        // must produce an "undefined variable" compile error.
        let err = compile_src("f: {(equal [x, 0]): (n: 0), _: n}").unwrap_err();
        assert!(
            err.contains("undefined variable: n"),
            "expected scope-isolation error; got: {err}"
        );
    }

    #[test]
    fn test_rename_body_can_still_use_renamed_var() {
        // Sanity-check that the fix doesn't break the intended use: `n` must
        // still be visible *inside* the rename body.
        assert!(
            compile_src("f: (n: (add [n, 1]))").is_ok(),
            "rename body should be able to reference the renamed variable"
        );
    }

    // ---- get compile errors --------------------------------------------------

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
        // The map argument is a local variable — shape is unknown at compile time.
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
