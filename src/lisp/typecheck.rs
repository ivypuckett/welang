//! Hindley-Milner type inference for welang, powered by the `polytype` crate.
//!
//! ## Type universe
//!
//! | Welang construct                  | Type                |
//! |-----------------------------------|---------------------|
//! | number literal / boolean literal  | `int`               |
//! | string literal                    | `str`               |
//! | `[a, b]` tuple                    | `tuple(T)` – both elements must be `T` |
//! | `{k: v, …}` map                   | `map(T)` – all values must be `T`      |
//! | function `name: body`             | `α → β`             |
//!
//! ## Rules applied during inference
//!
//! * Arithmetic ops (`add subtract multiply divide`) require `tuple(int)` and
//!   produce `int`.
//! * Comparison ops (`equal lessThan …`) require `tuple(int)` and produce
//!   `int`.
//! * `print n` requires `int` and returns `int`.
//! * `print "s"` accepts the string literal as-is and returns `int`.
//! * `get [map_expr, key]` requires the map to have type `map(T)` and returns
//!   `T`.
//! * A `{(cond): v, …, _: default}` conditional requires all branches to
//!   produce the same type.  Condition expressions are unconstrained (any
//!   non-zero value is truthy at runtime).
//! * A pipeline `(f3 f2 f1 x | f4)` is desugared by the parser into nested
//!   calls before this pass runs, so composition type-flow is checked
//!   automatically through function-application inference.
//! * In `[a, b]` tuples every element must share the same type.
//! * In `{k: v, …}` maps every value must share the same type.

use std::collections::HashMap;

use polytype::{Context, Type, TypeScheme};

use super::parser::Expr;

// ---------------------------------------------------------------------------
// Type aliases
// ---------------------------------------------------------------------------

/// A concrete (monomorphic) type, possibly still containing unresolved type
/// variables that will be resolved once all constraints have been accumulated.
pub type Ty = Type<&'static str>;

/// A (possibly polymorphic) type scheme, used for built-in operators.
type Scheme = TypeScheme<&'static str>;

/// The polytype unification context that tracks substitutions.
type Ctx = Context<&'static str>;

// ---------------------------------------------------------------------------
// Primitive type constructors
// ---------------------------------------------------------------------------

fn ty_int() -> Ty {
    Ty::Constructed("int", vec![])
}

fn ty_str() -> Ty {
    Ty::Constructed("str", vec![])
}

fn ty_map(elem: Ty) -> Ty {
    Ty::Constructed("map", vec![elem])
}

fn ty_tuple(elem: Ty) -> Ty {
    Ty::Constructed("tuple", vec![elem])
}

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

/// A type error produced during inference.
#[derive(Debug, Clone)]
pub struct TypeError {
    pub message: String,
}

impl std::fmt::Display for TypeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

/// Attempt to unify `t1` and `t2` in `ctx`, annotating any failure with
/// `context` for a human-readable error message.
fn unify(ctx: &mut Ctx, t1: &Ty, t2: &Ty, context: &str) -> Result<(), TypeError> {
    ctx.unify(t1, t2).map_err(|e| TypeError {
        message: format!("type mismatch in {context}: {e}"),
    })
}

// ---------------------------------------------------------------------------
// Per-function seeded types
// ---------------------------------------------------------------------------

/// For each user-defined function we seed a `(param_type, return_type)` pair
/// of fresh type variables in pass 1.  Pass 2 then infers the body and
/// unifies it with these variables.  Sharing the variables across the two
/// passes lets mutually-recursive functions propagate constraints to each
/// other automatically.
type FuncVars = HashMap<String, (Ty, Ty)>;

// ---------------------------------------------------------------------------
// Built-in type schemes
// ---------------------------------------------------------------------------

/// Return the static type scheme for a built-in operator, or `None` if the
/// name is not a built-in.
fn builtin_scheme(name: &str) -> Option<Scheme> {
    match name {
        "add" | "subtract" | "multiply" | "divide" => {
            // tuple(int) → int
            Some(Scheme::Monotype(Ty::arrow(ty_tuple(ty_int()), ty_int())))
        }
        "equal" | "equals" | "lessThan" | "greaterThan" | "lessThanOrEqual"
        | "greaterThanOrEqual" => {
            // tuple(int) → int
            Some(Scheme::Monotype(Ty::arrow(ty_tuple(ty_int()), ty_int())))
        }
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

/// Run type inference over a slice of top-level `(define …)` AST nodes.
///
/// Returns `Ok(())` if every function body is well-typed, or a [`TypeError`]
/// describing the first inconsistency found.
///
/// The check is performed in two passes:
///
/// 1. **Seed**: assign fresh type variables to every function's parameter and
///    return position so that mutually-recursive calls can be resolved.
/// 2. **Infer**: walk each function body, accumulate unification constraints,
///    and ensure the inferred return type matches the seeded return variable.
pub fn type_check(exprs: &[Expr]) -> Result<(), TypeError> {
    let mut ctx = Ctx::default();
    let mut func_vars: FuncVars = HashMap::new();

    // --- Pass 1: seed fresh type variables for every function ---------------
    for expr in exprs {
        if let Some((name, _params, _body)) = extract_define(expr) {
            let param_ty = ctx.new_variable();
            let ret_ty = ctx.new_variable();
            func_vars.insert(name.to_string(), (param_ty, ret_ty));
        }
    }

    // --- Pass 2: infer and unify --------------------------------------------
    for expr in exprs {
        if let Some((name, param_names, body)) = extract_define(expr) {
            let (param_ty, ret_ty) = func_vars[name].clone();

            // Build the local variable environment: bind each declared
            // parameter name to the function's seeded parameter type.
            let mut env: HashMap<String, Ty> = HashMap::new();
            for p in &param_names {
                env.insert((*p).to_string(), param_ty.clone());
            }

            let inferred = infer_expr(body, &env, &func_vars, &mut ctx, name)?;

            // Apply accumulated substitutions before unifying, so the error
            // message shows concrete types rather than raw type-variable IDs.
            let inferred = inferred.apply(&ctx);
            let ret = ret_ty.apply(&ctx);
            unify(
                &mut ctx,
                &inferred,
                &ret,
                &format!("return type of '{name}'"),
            )?;
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Helper: extract the components of a `(define (name params…) body)` node
// ---------------------------------------------------------------------------

fn extract_define(expr: &Expr) -> Option<(&str, Vec<&str>, &Expr)> {
    if let Expr::List(items) = expr
        && items.len() >= 3
        && let Expr::Symbol(kw) = &items[0]
        && kw == "define"
        && let Expr::List(sig) = &items[1]
        && let Some(Expr::Symbol(name)) = sig.first()
    {
        let param_names: Vec<&str> = sig[1..]
            .iter()
            .filter_map(|e| {
                if let Expr::Symbol(s) = e {
                    Some(s.as_str())
                } else {
                    None
                }
            })
            .collect();
        Some((name.as_str(), param_names, &items[2]))
    } else {
        None
    }
}

// ---------------------------------------------------------------------------
// Core type inference
// ---------------------------------------------------------------------------

fn infer_expr(
    expr: &Expr,
    env: &HashMap<String, Ty>,
    func_vars: &FuncVars,
    ctx: &mut Ctx,
    func_name: &str,
) -> Result<Ty, TypeError> {
    match expr {
        // ── Literals ─────────────────────────────────────────────────────
        // Numbers and booleans are both represented as i64 at runtime.
        Expr::Number(_) => Ok(ty_int()),
        Expr::Bool(_) => Ok(ty_int()),
        Expr::Str(_) => Ok(ty_str()),

        // ── Variable reference ───────────────────────────────────────────
        Expr::Symbol(name) => env.get(name.as_str()).cloned().ok_or_else(|| TypeError {
            message: format!("undefined variable '{name}' in function '{func_name}'"),
        }),

        // ── Tuple `[a, b, …]` ───────────────────────────────────────────
        // Every element must share the same type; the tuple type carries
        // that element type as a parameter.
        Expr::Tuple(elems) => {
            if elems.is_empty() {
                // An empty tuple is valid only as a degenerate case; give it
                // a fully-polymorphic element type.
                let elem_ty = ctx.new_variable();
                return Ok(ty_tuple(elem_ty));
            }
            let first_ty = infer_expr(&elems[0], env, func_vars, ctx, func_name)?;
            for (i, elem) in elems.iter().enumerate().skip(1) {
                let elem_ty = infer_expr(elem, env, func_vars, ctx, func_name)?;
                let first_applied = first_ty.apply(ctx);
                let elem_applied = elem_ty.apply(ctx);
                unify(
                    ctx,
                    &elem_applied,
                    &first_applied,
                    &format!("tuple element {i} in function '{func_name}'"),
                )?;
            }
            Ok(ty_tuple(first_ty.apply(ctx)))
        }

        // ── Map literal `{k: v, …}` ──────────────────────────────────────
        // All values must share the same type.
        Expr::Map(entries) => {
            if entries.is_empty() {
                let val_ty = ctx.new_variable();
                return Ok(ty_map(val_ty));
            }
            let first_ty = infer_expr(&entries[0].1, env, func_vars, ctx, func_name)?;
            for (i, (_key, val_expr)) in entries.iter().enumerate().skip(1) {
                let val_ty = infer_expr(val_expr, env, func_vars, ctx, func_name)?;
                let first_applied = first_ty.apply(ctx);
                let val_applied = val_ty.apply(ctx);
                unify(
                    ctx,
                    &val_applied,
                    &first_applied,
                    &format!("map value at position {i} in function '{func_name}'"),
                )?;
            }
            Ok(ty_map(first_ty.apply(ctx)))
        }

        // ── Conditional `{(cond): v, …, _: default}` ─────────────────────
        // Condition expressions are unconstrained (any non-zero value is
        // truthy).  All branch values must share the same return type.
        Expr::Cond(entries) => {
            let result_ty = ctx.new_variable();
            for (cond_opt, val_expr) in entries {
                if let Some(cond) = cond_opt {
                    // Infer (and thus check) the condition expression, but do
                    // not add any type constraint on it — any value is truthy
                    // when non-zero.
                    let _cond_ty = infer_expr(cond, env, func_vars, ctx, func_name)?;
                }
                let val_ty = infer_expr(val_expr, env, func_vars, ctx, func_name)?;
                let result_applied = result_ty.apply(ctx);
                let val_applied = val_ty.apply(ctx);
                unify(
                    ctx,
                    &val_applied,
                    &result_applied,
                    &format!("conditional branch in function '{func_name}'"),
                )?;
            }
            Ok(result_ty.apply(ctx))
        }

        // ── Rename binding `(new_name: body)` ────────────────────────────
        // Introduces `new_name` as an alias for the current `x` inside
        // `body`.
        Expr::Rename(new_name, body) => {
            let x_ty = env.get("x").cloned().ok_or_else(|| TypeError {
                message: format!("rename '({new_name}: …)' can only be used where 'x' is in scope"),
            })?;
            let mut inner_env = env.clone();
            inner_env.insert(new_name.clone(), x_ty);
            infer_expr(body, &inner_env, func_vars, ctx, func_name)
        }

        // ── Empty list `()` ──────────────────────────────────────────────
        Expr::List(items) if items.is_empty() => Err(TypeError {
            message: "empty list () is not a valid expression".to_string(),
        }),

        // ── Grouped expression `(expr)` ──────────────────────────────────
        Expr::List(items) if items.len() == 1 => {
            infer_expr(&items[0], env, func_vars, ctx, func_name)
        }

        // ── Function application / built-in call ─────────────────────────
        Expr::List(items) => match &items[0] {
            Expr::Symbol(op)
                if matches!(op.as_str(), "add" | "subtract" | "multiply" | "divide") =>
            {
                infer_arith_op(op, &items[1..], env, func_vars, ctx, func_name)
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
                infer_arith_op(op, &items[1..], env, func_vars, ctx, func_name)
            }

            Expr::Symbol(kw) if kw == "print" => {
                infer_print(&items[1..], env, func_vars, ctx, func_name)
            }

            Expr::Symbol(kw) if kw == "get" => {
                infer_get(&items[1..], env, func_vars, ctx, func_name)
            }

            Expr::Symbol(name) => infer_call(name, &items[1..], env, func_vars, ctx, func_name),

            other => Err(TypeError {
                message: format!("cannot call {other:?} as a function"),
            }),
        },

        Expr::Quote(_) => Err(TypeError {
            message: "quoted expressions are not supported in compiled code".to_string(),
        }),
    }
}

// ---------------------------------------------------------------------------
// Built-in: arithmetic and comparison operators
// ---------------------------------------------------------------------------

/// Infer the type of `(op [a, b])`.
///
/// Both `a` and `b` must be `int`; the result is `int`.
fn infer_arith_op(
    op: &str,
    args: &[Expr],
    env: &HashMap<String, Ty>,
    func_vars: &FuncVars,
    ctx: &mut Ctx,
    func_name: &str,
) -> Result<Ty, TypeError> {
    // Validate structure: exactly one 2-element tuple argument.
    let (lhs, rhs) = match args {
        [Expr::Tuple(elems)] if elems.len() == 2 => (&elems[0], &elems[1]),
        _ => {
            return Err(TypeError {
                message: format!("'{op}' requires a 2-element tuple argument [a, b]"),
            });
        }
    };

    let lhs_ty = infer_expr(lhs, env, func_vars, ctx, func_name)?.apply(ctx);
    let rhs_ty = infer_expr(rhs, env, func_vars, ctx, func_name)?.apply(ctx);

    unify(
        ctx,
        &lhs_ty,
        &ty_int(),
        &format!("left-hand side of '{op}' in function '{func_name}'"),
    )?;
    unify(
        ctx,
        &rhs_ty,
        &ty_int(),
        &format!("right-hand side of '{op}' in function '{func_name}'"),
    )?;

    // Use the built-in scheme just to make polytype aware of it (good
    // practice), even though we've already constrained the operands above.
    let _scheme = builtin_scheme(op);

    Ok(ty_int())
}

// ---------------------------------------------------------------------------
// Built-in: print
// ---------------------------------------------------------------------------

/// Infer the type of `(print arg)`.
///
/// * `(print "s")` — the string literal is accepted as-is; returns `int`.
/// * `(print n)`   — `n` must be `int`; returns `int`.
fn infer_print(
    args: &[Expr],
    env: &HashMap<String, Ty>,
    func_vars: &FuncVars,
    ctx: &mut Ctx,
    func_name: &str,
) -> Result<Ty, TypeError> {
    if args.len() != 1 {
        return Err(TypeError {
            message: format!("'print' expects 1 argument, got {}", args.len()),
        });
    }
    match &args[0] {
        // String literal: accepted without further constraint.
        Expr::Str(_) => Ok(ty_int()),
        // Any other expression: must be int.
        other => {
            let arg_ty = infer_expr(other, env, func_vars, ctx, func_name)?.apply(ctx);
            unify(
                ctx,
                &arg_ty,
                &ty_int(),
                &format!("argument of 'print' in function '{func_name}'"),
            )?;
            Ok(ty_int())
        }
    }
}

// ---------------------------------------------------------------------------
// Built-in: get
// ---------------------------------------------------------------------------

/// Infer the type of `(get [map_expr, key])`.
///
/// The map expression must have type `map(T)` for some `T`; the result is
/// `T`.  The key is a compile-time symbol or string; it does not carry its
/// own polymorphic type beyond being used to index into the map.
fn infer_get(
    args: &[Expr],
    env: &HashMap<String, Ty>,
    func_vars: &FuncVars,
    ctx: &mut Ctx,
    func_name: &str,
) -> Result<Ty, TypeError> {
    let (map_expr, _key_expr) = match args {
        [Expr::Tuple(elems)] if elems.len() == 2 => (&elems[0], &elems[1]),
        _ => {
            return Err(TypeError {
                message: "'get' requires a 2-element tuple argument [map, key]".to_string(),
            });
        }
    };

    let map_ty = infer_expr(map_expr, env, func_vars, ctx, func_name)?.apply(ctx);

    // Introduce a fresh type variable for the value type T.
    let val_ty = ctx.new_variable();
    let expected = ty_map(val_ty.clone());

    unify(
        ctx,
        &map_ty,
        &expected,
        &format!("'get' map argument in function '{func_name}'"),
    )?;

    Ok(val_ty.apply(ctx))
}

// ---------------------------------------------------------------------------
// User-defined function call
// ---------------------------------------------------------------------------

/// Infer the type of a call `(name arg?)`.
///
/// All user-defined functions are single-argument (implicit `x`).  Calling
/// with no argument is also accepted (some functions ignore `x`), in which
/// case the argument type is left as a fresh unconstrained variable so that
/// the callee's parameter type is not pinned.
fn infer_call(
    name: &str,
    args: &[Expr],
    env: &HashMap<String, Ty>,
    func_vars: &FuncVars,
    ctx: &mut Ctx,
    func_name: &str,
) -> Result<Ty, TypeError> {
    let (param_ty, ret_ty) = func_vars.get(name).cloned().ok_or_else(|| TypeError {
        message: format!("undefined function '{name}' (called from '{func_name}')"),
    })?;

    let arg_ty = match args {
        [] => {
            // Zero-argument call: the callee ignores its parameter.  Use a
            // fresh variable so we don't over-constrain the callee.
            ctx.new_variable()
        }
        [arg] => infer_expr(arg, env, func_vars, ctx, func_name)?,
        _ => {
            return Err(TypeError {
                message: format!("function '{name}' takes 1 argument but got {}", args.len()),
            });
        }
    };

    let arg_applied = arg_ty.apply(ctx);
    let param_applied = param_ty.apply(ctx);
    unify(
        ctx,
        &arg_applied,
        &param_applied,
        &format!("argument passed to '{name}' from '{func_name}'"),
    )?;

    Ok(ret_ty.apply(ctx))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lisp::parser::parse;

    fn check(src: &str) -> Result<(), TypeError> {
        let exprs = parse(src).expect("parse should succeed");
        type_check(&exprs)
    }

    fn check_err(src: &str) -> String {
        check(src).unwrap_err().message
    }

    // ── basics ────────────────────────────────────────────────────────────

    #[test]
    fn test_hello() {
        assert!(check("main: 0").is_ok());
    }

    #[test]
    fn test_arithmetic() {
        assert!(check("main: (add [1, 2])").is_ok());
    }

    #[test]
    fn test_comparison() {
        assert!(check("main: (lessThan [1, 2])").is_ok());
    }

    #[test]
    fn test_cond() {
        assert!(check("f: {(lessThan [x, 0]): 1, _: 0}").is_ok());
    }

    #[test]
    fn test_map_homogeneous() {
        assert!(check("main: {x: 1, y: 2}").is_ok());
    }

    // ── type errors ────────────────────────────────────────────────────────

    #[test]
    fn test_arith_with_string_lhs() {
        let msg = check_err(r#"main: (add ["hello", 1])"#);
        assert!(
            msg.contains("mismatch"),
            "expected type mismatch, got: {msg}"
        );
    }

    #[test]
    fn test_tuple_mixed_types() {
        // Concrete incompatible literals: int vs str.
        let msg = check_err(r#"f: [1, "hello"]"#);
        assert!(
            msg.contains("mismatch"),
            "expected type mismatch, got: {msg}"
        );
    }

    #[test]
    fn test_map_mixed_value_types() {
        let msg = check_err(r#"main: {a: 1, b: "hello"}"#);
        assert!(
            msg.contains("mismatch"),
            "expected type mismatch, got: {msg}"
        );
    }

    #[test]
    fn test_cond_branch_type_mismatch() {
        // Both arms must return the same type.
        let msg = check_err(r#"main: {(equal [0, 0]): 1, _: "nope"}"#);
        assert!(
            msg.contains("mismatch"),
            "expected type mismatch, got: {msg}"
        );
    }

    #[test]
    fn test_print_str_arg_ok() {
        assert!(check(r#"main: (print "hello")"#).is_ok());
    }

    #[test]
    fn test_print_int_arg_ok() {
        assert!(check("main: (print 42)").is_ok());
    }

    #[test]
    fn test_print_non_int_is_error() {
        // Trying to print a map is a type error.
        let msg = check_err("main: (print {a: 1})");
        assert!(
            msg.contains("mismatch"),
            "expected type mismatch, got: {msg}"
        );
    }

    #[test]
    fn test_recursion_ok() {
        let src = r#"
            factorial:
              {(lessThanOrEqual [x, 1]):
                1,
              _: (multiply [x, (factorial (subtract [x, 1]))])}
            main: 0
        "#;
        assert!(check(src).is_ok());
    }

    #[test]
    fn test_function_composition_ok() {
        let src = r#"
            double: (multiply [2, x])
            quadruple: (double (double x))
            main: 0
        "#;
        assert!(check(src).is_ok());
    }

    #[test]
    fn test_pipe_type_mismatch() {
        // double expects int, but we pass a string.
        let src = r#"
            double: (multiply [2, x])
            main: (double "hello")
        "#;
        let msg = check_err(src);
        assert!(
            msg.contains("mismatch"),
            "expected type mismatch, got: {msg}"
        );
    }

    #[test]
    fn test_get_map_ok() {
        assert!(check("main: (get [{x: 10, y: 20}, x])").is_ok());
    }

    #[test]
    fn test_get_non_map_is_error() {
        let msg = check_err("main: (get [42, x])");
        assert!(
            msg.contains("mismatch"),
            "expected type mismatch, got: {msg}"
        );
    }

    #[test]
    fn test_rename_ok() {
        assert!(check("abs: (n: {(lessThan [n, 0]): (subtract [0, n]), _: n})").is_ok());
    }

    #[test]
    fn test_undefined_function() {
        let msg = check_err("main: (notAFunction 1)");
        assert!(
            msg.contains("undefined function"),
            "expected undefined-function error, got: {msg}"
        );
    }
}
