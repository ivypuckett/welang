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
    /// `strcmp(s1: i64, s2: i64) -> i32` — used by `(get ...)` for key lookup.
    strcmp_id: FuncId,
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
///   `(op [a, b])`               — arithmetic: `+  -  *  /`
///   `(op [a, b])`               — comparison: `=  <  >  <=  >=`  (returns 0 or 1)
///   `(if [cond, then])`         — conditional (else = 0)
///   `(if [cond, then, else])`   — conditional with else branch
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

    // Declare strcmp(s1: i64, s2: i64) -> i32 for map key lookup.
    let strcmp_id = {
        let mut sig = module.make_signature();
        sig.params.push(AbiParam::new(types::I64)); // s1 pointer
        sig.params.push(AbiParam::new(types::I64)); // s2 pointer
        sig.returns.push(AbiParam::new(types::I32)); // comparison result
        module
            .declare_function("strcmp", Linkage::Import, &sig)
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
        strcmp_id,
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
            compile_function(&mut module, &registry, sig_items, &items[2..], &builtins)?;
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

        let mut result = builder.ins().iconst(types::I64, 0);
        for expr in body {
            result = compile_expr(
                &mut builder,
                module,
                registry,
                expr,
                &mut locals,
                &mut next_var,
                builtins,
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
            compile_expr(builder, module, registry, body, locals, next_var, builtins)
        }

        // Tuple at expression level is only valid as an argument to an operator.
        Expr::Tuple(_) => Err(
            "tuple literal [...] can only appear as an argument to a built-in operator".to_string(),
        ),

        Expr::Map(entries) => compile_map(
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
            Expr::Symbol(op) if matches!(op.as_str(), "+" | "-" | "*" | "/") => {
                let (lhs, rhs) = unpack_binary_tuple(op, &items[1..])?;
                compile_arith(
                    builder, module, registry, op, lhs, rhs, locals, next_var, builtins,
                )
            }
            Expr::Symbol(op) if matches!(op.as_str(), "=" | "<" | ">" | "<=" | ">=") => {
                let (lhs, rhs) = unpack_binary_tuple(op, &items[1..])?;
                compile_cmp(
                    builder, module, registry, op, lhs, rhs, locals, next_var, builtins,
                )
            }
            Expr::Symbol(kw) if kw == "if" => compile_if(
                builder,
                module,
                registry,
                &items[1..],
                locals,
                next_var,
                builtins,
            ),
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

        Expr::Quote(_) => Err("quoted expressions are not supported in compiled code".to_string()),
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
    builtins: &Builtins,
) -> Result<Value, CompileError> {
    let lv = compile_expr(builder, module, registry, lhs, locals, next_var, builtins)?;
    let rv = compile_expr(builder, module, registry, rhs, locals, next_var, builtins)?;
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

#[allow(clippy::too_many_arguments)]
fn compile_if(
    builder: &mut FunctionBuilder,
    module: &mut ObjectModule,
    registry: &HashMap<String, FuncInfo>,
    args: &[Expr],
    locals: &mut HashMap<String, Variable>,
    next_var: &mut usize,
    builtins: &Builtins,
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

    let cond = compile_expr(
        builder,
        module,
        registry,
        &tuple_elems[0],
        locals,
        next_var,
        builtins,
    )?;
    let zero = builder.ins().iconst(types::I64, 0);
    let flag = builder.ins().icmp(IntCC::NotEqual, cond, zero);

    let then_block = builder.create_block();
    let else_block = builder.create_block();
    let merge_block = builder.create_block();
    builder.append_block_param(merge_block, types::I64);

    builder.ins().brif(flag, then_block, &[], else_block, &[]);

    builder.switch_to_block(then_block);
    builder.seal_block(then_block);
    let then_val = compile_expr(
        builder,
        module,
        registry,
        &tuple_elems[1],
        locals,
        next_var,
        builtins,
    )?;
    builder.ins().jump(merge_block, &[then_val]);

    builder.switch_to_block(else_block);
    builder.seal_block(else_block);
    let else_val = if tuple_elems.len() == 3 {
        compile_expr(
            builder,
            module,
            registry,
            &tuple_elems[2],
            locals,
            next_var,
            builtins,
        )?
    } else {
        builder.ins().iconst(types::I64, 0)
    };
    builder.ins().jump(merge_block, &[else_val]);

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
/// `map` is any expression that evaluates to a map pointer.
/// `key` is a symbol or string literal naming the field to retrieve.
///
/// Performs a runtime linear scan of the map's key list using `strcmp`.
/// Returns the matching value, or `0` if the key is not found.
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
        _ => return Err("map key must be a symbol or string literal".to_string()),
    };

    // Compile the map pointer and intern the target key string.
    let map_ptr = compile_expr(
        builder, module, registry, map_expr, locals, next_var, builtins,
    )?;
    let target_key = intern_string(module, builder, builtins, &key_name, "__we_getkey_")?;

    // Load the number of fields from the map header.
    let n_fields = builder.ins().load(types::I64, MemFlags::new(), map_ptr, 0);

    // ── block layout ──────────────────────────────────────────────────────────
    // loop_header(i: i64) — entry point; checks i < n_fields
    // loop_body            — loads stored key and calls strcmp
    // loop_found           — loads matching value, jumps to merge
    // loop_continue        — increments i, jumps back to loop_header
    // loop_exit            — n_fields reached, key not found
    // merge(result: i64)   — join point
    let loop_header = builder.create_block();
    let loop_body = builder.create_block();
    let loop_found = builder.create_block();
    let loop_continue = builder.create_block();
    let loop_exit = builder.create_block();
    let merge_block = builder.create_block();

    builder.append_block_param(loop_header, types::I64); // loop variable i
    builder.append_block_param(merge_block, types::I64); // result

    // Jump into the loop with i = 0.
    let zero = builder.ins().iconst(types::I64, 0);
    builder.ins().jump(loop_header, &[zero]);

    // loop_header: exit if i >= n_fields.
    builder.switch_to_block(loop_header);
    // (sealed later, after the back-edge from loop_continue is established)
    let i = builder.block_params(loop_header)[0];
    let done = builder
        .ins()
        .icmp(IntCC::SignedGreaterThanOrEqual, i, n_fields);
    builder.ins().brif(done, loop_exit, &[], loop_body, &[]);

    // loop_body: load the stored key pointer and strcmp it.
    builder.switch_to_block(loop_body);
    builder.seal_block(loop_body);
    {
        let two = builder.ins().iconst(types::I64, 2);
        let eight = builder.ins().iconst(types::I64, 8);
        let one = builder.ins().iconst(types::I64, 1);
        let i2 = builder.ins().imul(i, two);
        let idx = builder.ins().iadd(one, i2); // 1 + 2*i
        let key_byte_off = builder.ins().imul(idx, eight); // (1 + 2*i) * 8
        let key_field_addr = builder.ins().iadd(map_ptr, key_byte_off);
        let stored_key = builder
            .ins()
            .load(types::I64, MemFlags::new(), key_field_addr, 0);

        let strcmp_ref = module.declare_func_in_func(builtins.strcmp_id, builder.func);
        let cmp_call = builder.ins().call(strcmp_ref, &[stored_key, target_key]);
        let cmp_i32 = builder.inst_results(cmp_call)[0];
        let matched = builder.ins().icmp_imm(IntCC::Equal, cmp_i32, 0);
        builder
            .ins()
            .brif(matched, loop_found, &[], loop_continue, &[]);
    }

    // loop_found: recompute value address (using i from loop_header, which
    // dominates this block) and jump to merge.
    builder.switch_to_block(loop_found);
    builder.seal_block(loop_found);
    {
        let two = builder.ins().iconst(types::I64, 2);
        let eight = builder.ins().iconst(types::I64, 8);
        let two_i = builder.ins().imul(i, two); // 2 * i
        let idx = builder.ins().iadd(two_i, two); // 2 + 2*i  (= val slot index)
        let val_byte_off = builder.ins().imul(idx, eight);
        let val_addr = builder.ins().iadd(map_ptr, val_byte_off);
        let val = builder.ins().load(types::I64, MemFlags::new(), val_addr, 0);
        builder.ins().jump(merge_block, &[val]);
    }

    // loop_continue: advance i and loop back.
    builder.switch_to_block(loop_continue);
    builder.seal_block(loop_continue);
    {
        let one = builder.ins().iconst(types::I64, 1);
        let next_i = builder.ins().iadd(i, one);
        builder.ins().jump(loop_header, &[next_i]);
    }
    // All predecessors of loop_header are now known.
    builder.seal_block(loop_header);

    // loop_exit: key not found — return 0.
    builder.switch_to_block(loop_exit);
    builder.seal_block(loop_exit);
    {
        let zero2 = builder.ins().iconst(types::I64, 0);
        builder.ins().jump(merge_block, &[zero2]);
    }

    // merge: collect the result.
    builder.switch_to_block(merge_block);
    builder.seal_block(merge_block);
    Ok(builder.block_params(merge_block)[0])
}
