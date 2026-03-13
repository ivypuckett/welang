//! Hindley-Milner type inference for welang, powered by the `polytype` crate.
//!
//! ## Type universe
//!
//! | Welang construct                  | Type                |
//! |-----------------------------------|---------------------|
//! | number literal / boolean literal  | `int`               |
//! | string literal                    | `str`               |
//! | `[a, b]` tuple                    | `tuple(T)` – both elements must be `T` |
//! | `{k: v, …}` map                   | `map(α)` – values may differ; all must accept the same input type |
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
//! * In `{k: v, …}` maps every value expression must accept the same input type
///   (i.e. the type of `x` must be consistent), but the values themselves may
///   have different types.  The map type is `map(α)` where `α` is a fresh
///   unconstrained variable.
use std::collections::{HashMap, HashSet};

use polytype::{Context, Type, TypeScheme};

use super::parser::{Expr, TypeExpr};

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

/// In-scope generic type parameter bindings, built when descending into a
/// `TypeExpr::Generic` node.  Maps each type-parameter name (e.g. `"T"`) to
/// the concrete `Ty` it was resolved to.
type GenericBindings = HashMap<String, Ty>;

/// Map from structural-type function name to its original `TypeExpr`.
/// Used to specialize generic types when they are referenced by name inside
/// another generic body (e.g. `'<T i64> y` where `y: '<T _>T`).
type TypeExprs = HashMap<String, TypeExpr>;

/// Set of names that were declared as nominal types (`name: *typeExpr`).
/// Nominal types require that any annotated function body calling a function
/// must call the nominal constructor explicitly.
type NominalTypes = HashSet<String>;

// ---------------------------------------------------------------------------
// Built-in type schemes
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Structural type → Ty conversion
// ---------------------------------------------------------------------------

/// Convert a `TypeExpr` to a concrete `Ty`, treating `Function(A, B)` as its
/// output type `B` (the scalar return type of the function).
///
/// Primitive types `i64`, `f64`, and `bool` all lower to `int` because they
/// share the same 64-bit machine representation.  Unknown user-defined type
/// names are first looked up in `generics` (in-scope generic type parameters),
/// then in `func_vars` (the named structural-type function); if not found a
/// fresh type variable is returned so the type remains polymorphic.
fn typeexpr_to_scalar(
    ty: &TypeExpr,
    ctx: &mut Ctx,
    func_vars: &FuncVars,
    generics: &GenericBindings,
    type_exprs: &TypeExprs,
) -> Ty {
    match ty {
        TypeExpr::Named(name) => match name.as_str() {
            "i64" | "f64" | "bool" | "int" => ty_int(),
            "str" | "string" => ty_str(),
            _ => {
                // Generic type parameter takes priority over any global name.
                if let Some(ty) = generics.get(name.as_str()) {
                    ty.clone()
                } else if let Some(te) = type_exprs.get(name.as_str()) {
                    // Named structural type: apply current generic bindings as
                    // specialization arguments to the type's own parameters.
                    specialize_scalar(te, ctx, func_vars, generics, type_exprs)
                } else if let Some((_, ret_ty)) = func_vars.get(name.as_str()) {
                    ret_ty.clone()
                } else {
                    ctx.new_variable()
                }
            }
        },
        TypeExpr::Wildcard => ctx.new_variable(),
        TypeExpr::Array(elem) => ty_tuple(typeexpr_to_scalar(
            elem, ctx, func_vars, generics, type_exprs,
        )),
        TypeExpr::Map(_) => ty_map(ctx.new_variable()),
        // For function types used in scalar context, return the output type.
        TypeExpr::Function(_, output) => {
            typeexpr_to_scalar(output, ctx, func_vars, generics, type_exprs)
        }
        // Generic type: resolve each param to its constraint type (or a fresh
        // variable for wildcards), extend the bindings, then convert the body.
        TypeExpr::Generic(params, body) => {
            let mut extended = generics.clone();
            for (name, constraint) in params {
                let param_ty =
                    typeexpr_to_scalar(constraint, ctx, func_vars, &extended, type_exprs);
                extended.insert(name.clone(), param_ty);
            }
            typeexpr_to_scalar(body, ctx, func_vars, &extended, type_exprs)
        }
        // Nominal type: same HM representation as the inner structural type.
        TypeExpr::Nominal(inner) => typeexpr_to_scalar(inner, ctx, func_vars, generics, type_exprs),
    }
}

/// Specialize a named structural type's `TypeExpr` using the current
/// `outer_generics` bindings.
///
/// When a generic body references another generic type by name (e.g.
/// `'<T i64> y` where `y: '<T _>T`), the outer type parameters should be
/// used to instantiate the named type's own parameters (matched by name).
/// Any of the named type's parameters that do not appear in `outer_generics`
/// fall back to their own constraint.
fn specialize_scalar(
    ty: &TypeExpr,
    ctx: &mut Ctx,
    func_vars: &FuncVars,
    outer_generics: &GenericBindings,
    type_exprs: &TypeExprs,
) -> Ty {
    if let TypeExpr::Generic(params, body) = ty {
        let mut bindings = GenericBindings::new();
        for (pname, constraint) in params {
            if let Some(outer_ty) = outer_generics.get(pname.as_str()) {
                // Specialise: use the caller's binding for this param.
                bindings.insert(pname.clone(), outer_ty.clone());
            } else {
                // No outer binding: evaluate from the param's own constraint.
                let t = typeexpr_to_scalar(constraint, ctx, func_vars, outer_generics, type_exprs);
                bindings.insert(pname.clone(), t);
            }
        }
        typeexpr_to_scalar(body, ctx, func_vars, &bindings, type_exprs)
    } else {
        typeexpr_to_scalar(ty, ctx, func_vars, outer_generics, type_exprs)
    }
}

/// Decompose a `TypeExpr` into `(param_type, return_type)`.
///
/// For `Function(A, B)` this returns `(A_ty, B_ty)`.  For all other types
/// the same scalar type is used for both positions (identity function).
fn typeexpr_to_param_ret(
    ty: &TypeExpr,
    ctx: &mut Ctx,
    func_vars: &FuncVars,
    generics: &GenericBindings,
    type_exprs: &TypeExprs,
) -> (Ty, Ty) {
    match ty {
        TypeExpr::Function(input, output) => {
            let param = typeexpr_to_scalar(input, ctx, func_vars, generics, type_exprs);
            let ret = typeexpr_to_scalar(output, ctx, func_vars, generics, type_exprs);
            (param, ret)
        }
        // Generic type: resolve params, then decompose the body.
        TypeExpr::Generic(params, body) => {
            let mut extended = generics.clone();
            for (name, constraint) in params {
                let param_ty =
                    typeexpr_to_scalar(constraint, ctx, func_vars, &extended, type_exprs);
                extended.insert(name.clone(), param_ty);
            }
            typeexpr_to_param_ret(body, ctx, func_vars, &extended, type_exprs)
        }
        other => {
            let scalar = typeexpr_to_scalar(other, ctx, func_vars, generics, type_exprs);
            (scalar.clone(), scalar)
        }
    }
}

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

    // Collect the original TypeExpr for every structural-type or nominal-type
    // function so that generic specialization can substitute outer bindings
    // into named type's own parameters.  For nominal types the inner TypeExpr
    // (without the `*` wrapper) is stored so that referencing them by name
    // works the same as referencing a structural type.
    let mut type_exprs: TypeExprs = HashMap::new();
    let mut nominal_types: NominalTypes = HashSet::new();
    for expr in exprs {
        if let Some((name, _, body, _)) = extract_define(expr) {
            match body {
                Expr::StructuralType(te) => {
                    type_exprs.insert(name.to_string(), te.clone());
                }
                Expr::NominalType(te) => {
                    // Store the inner TypeExpr (without the `*` wrapper) so
                    // that name-based specialization works identically.
                    type_exprs.insert(name.to_string(), te.clone());
                    nominal_types.insert(name.to_string());
                }
                _ => {}
            }
        }
    }

    // --- Pass 1: seed fresh type variables for every function ---------------
    //
    // For structural-type and nominal-type function bodies we can seed
    // concrete types immediately from the TypeExpr instead of fresh variables.
    // This lets call-sites see the exact type constraint from the start.
    for expr in exprs {
        if let Some((name, _params, body, _ann)) = extract_define(expr) {
            let type_expr_opt = match body {
                Expr::StructuralType(te) | Expr::NominalType(te) => Some(te),
                _ => None,
            };
            if let Some(type_expr) = type_expr_opt {
                // Use a temporary empty func_vars to convert the TypeExpr
                // (user-defined type names in it will become fresh variables here;
                // they are resolved properly in Pass 2 if needed).
                let empty: FuncVars = HashMap::new();
                let (param_ty, ret_ty) = typeexpr_to_param_ret(
                    type_expr,
                    &mut ctx,
                    &empty,
                    &HashMap::new(),
                    &type_exprs,
                );
                func_vars.insert(name.to_string(), (param_ty, ret_ty));
            } else {
                let param_ty = ctx.new_variable();
                let ret_ty = ctx.new_variable();
                func_vars.insert(name.to_string(), (param_ty, ret_ty));
            }
        }
    }

    // --- Pass 2: infer and unify --------------------------------------------
    for expr in exprs {
        if let Some((name, param_names, body, annotation)) = extract_define(expr) {
            let (param_ty, ret_ty) = func_vars[name].clone();

            // Build the local variable environment: bind each declared
            // parameter name to the function's seeded parameter type.
            let mut env: HashMap<String, Ty> = HashMap::new();
            for p in &param_names {
                env.insert((*p).to_string(), param_ty.clone());
            }

            // Infer the return type of the body.
            //
            // Structural-type bodies (`'type`) and nominal-type bodies (`*type`)
            // are handled specially: the body IS the type descriptor; the
            // function is an identity — it returns its argument unchanged, so
            // the return type equals the param type.
            let inferred =
                if let Expr::StructuralType(type_expr) | Expr::NominalType(type_expr) = body {
                    let (ann_param, ann_ret) = typeexpr_to_param_ret(
                        type_expr,
                        &mut ctx,
                        &func_vars,
                        &HashMap::new(),
                        &type_exprs,
                    );
                    let param_applied = param_ty.apply(&ctx);
                    unify(
                        &mut ctx,
                        &param_applied,
                        &ann_param,
                        &format!("structural type param of '{name}'"),
                    )?;
                    ann_ret
                } else {
                    infer_expr(body, &env, &func_vars, &mut ctx, name)?
                };

            // Apply any explicit type annotation.
            //
            // `name typeRef: body`        — named type annotation
            // `name 'typeExpr: body`      — inline structural type annotation
            // `name <T C>typeRef: body`   — specialized generic annotation
            if let Some(ann) = annotation {
                match ann {
                    Expr::Symbol(type_name) => {
                        // Named type ref: constrain return type to that type's
                        // return type.
                        if let Some((_, ann_ret_ty)) = func_vars.get(type_name.as_str()) {
                            let ann_ret = ann_ret_ty.apply(&ctx);
                            let inferred_applied = inferred.apply(&ctx);
                            unify(
                                &mut ctx,
                                &inferred_applied,
                                &ann_ret,
                                &format!("type annotation '{type_name}' on '{name}'"),
                            )?;
                        } else {
                            return Err(TypeError {
                                message: format!(
                                    "undefined type '{type_name}' used as annotation for '{name}'"
                                ),
                            });
                        }
                        // Nominal type additional check: when the annotation is a
                        // nominal type, any call expression in the body must go
                        // through the nominal constructor.  Bare literals and
                        // non-call expressions are allowed.
                        if nominal_types.contains(type_name.as_str()) {
                            check_nominal_body(body, type_name)?;
                        }
                    }
                    Expr::StructuralType(type_expr) => {
                        // Inline or specialized-generic type annotation:
                        // constrain both param and return.
                        let (ann_param, ann_ret) = typeexpr_to_param_ret(
                            type_expr,
                            &mut ctx,
                            &func_vars,
                            &HashMap::new(),
                            &type_exprs,
                        );
                        let param_applied = param_ty.apply(&ctx);
                        unify(
                            &mut ctx,
                            &param_applied,
                            &ann_param,
                            &format!("input type annotation on '{name}'"),
                        )?;
                        let inferred_applied = inferred.apply(&ctx);
                        unify(
                            &mut ctx,
                            &inferred_applied,
                            &ann_ret,
                            &format!("return type annotation on '{name}'"),
                        )?;
                    }
                    _ => {} // other annotation forms are ignored
                }
            }

            // Apply accumulated substitutions before final unification.
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

/// Returns `(name, param_names, body, optional_annotation)` for a
/// `(define (name params…) body)` or `(define (name params…) body annotation)` node.
fn extract_define(expr: &Expr) -> Option<(&str, Vec<&str>, &Expr, Option<&Expr>)> {
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
        let annotation = items.get(3);
        Some((name.as_str(), param_names, &items[2], annotation))
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
        // Local variable first; fall back to top-level function names, which
        // are first-class values represented as integer function pointers.
        Expr::Symbol(name) => {
            if let Some(ty) = env.get(name.as_str()) {
                Ok(ty.clone())
            } else if func_vars.contains_key(name.as_str()) {
                Ok(ty_int())
            } else {
                Err(TypeError {
                    message: format!("undefined variable '{name}' in function '{func_name}'"),
                })
            }
        }

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
        // Values may have different types; the only constraint is that every
        // value expression must accept the same input type for `x` (which is
        // already guaranteed by sharing the same `env`).  We still infer each
        // value so that `x`-type constraints from individual entries are
        // accumulated and unified through the shared environment.
        Expr::Map(entries) => {
            for (_key, val_expr) in entries {
                infer_expr(val_expr, env, func_vars, ctx, func_name)?;
            }
            // The map's value type is left as a fresh unconstrained variable
            // because entries may return different types.
            let val_ty = ctx.new_variable();
            Ok(ty_map(val_ty))
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

        // ── Structural type expression `'type` ───────────────────────────
        // In expression position a structural type evaluates to a type-tag
        // integer (0 at runtime).  Its primary use is either as the body of a
        // type-assertion function (`anyFloat: 'f64`) or as a type annotation
        // on a definition; both cases are handled in `type_check` before this
        // point.  When it appears as a plain value we give it type `int`.
        Expr::StructuralType(_) => Ok(ty_int()),

        // ── Nominal type expression `*type` ──────────────────────────────
        // Only valid as the body of a top-level definition (handled above).
        // If it appears elsewhere (e.g. as the callee of a call) the outer
        // list-matching code rejects it before we get here.  In the unlikely
        // event it reaches this branch, treat it as `int` for graceful handling.
        Expr::NominalType(_) => Ok(ty_int()),
    }
}

// ---------------------------------------------------------------------------
// Nominal type constructor enforcement
// ---------------------------------------------------------------------------

/// When a function is annotated with a nominal type `N`, any call expression
/// in the body must invoke the nominal constructor directly.  Bare literals
/// (numbers, booleans, strings), symbols, tuples, maps, and conditionals are
/// left unrestricted so that e.g. `z specialInt: 1` or `z specialInt: x` are
/// valid.
///
/// The rule: if `body` is an `Expr::List` (a parenthesised call), then its
/// first element must be `Expr::Symbol(nominal_name)`.
fn check_nominal_body(body: &Expr, nominal_name: &str) -> Result<(), TypeError> {
    if let Expr::List(items) = body {
        match items.first() {
            Some(Expr::Symbol(s)) if s == nominal_name => Ok(()),
            _ => Err(TypeError {
                message: format!(
                    "nominal type '{nominal_name}' requires the body to call the constructor; \
                     use ({nominal_name} ...) instead"
                ),
            }),
        }
    } else {
        Ok(())
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
    // Direct call: the name refers to a known top-level function.
    if let Some((param_ty, ret_ty)) = func_vars.get(name).cloned() {
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

        return Ok(ret_ty.apply(ctx));
    }

    // Indirect call: the name is a local variable holding a function pointer.
    // All user-defined functions are `int -> int` at runtime.
    if env.contains_key(name) {
        if args.len() != 1 {
            return Err(TypeError {
                message: format!(
                    "indirect call via '{name}' requires exactly 1 argument, got {}",
                    args.len()
                ),
            });
        }
        // Infer the argument to catch any inner type errors.
        infer_expr(&args[0], env, func_vars, ctx, func_name)?;
        return Ok(ty_int());
    }

    Err(TypeError {
        message: format!("undefined function '{name}' (called from '{func_name}')"),
    })
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
    fn test_map_heterogeneous_values_ok() {
        // Values with different types (str and int) are now allowed because both
        // entries accept the same input type for `x` (unconstrained here).
        assert!(check(r#"f: {label: "hello", count: x}"#).is_ok());
    }

    #[test]
    fn test_map_mixed_value_types_ok() {
        // A map whose values are a string literal and an int literal is valid:
        // neither entry constrains `x`, so the input type is consistent.
        assert!(check(r#"main: {a: 1, b: "hello"}"#).is_ok());
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

    // ── structural types ────────────────────────────────────────────────────

    #[test]
    fn test_structural_type_def_ok() {
        // `anyInt: 'i64` — structural type function definition
        assert!(check("anyInt: 'i64\nmain: 0").is_ok());
    }

    #[test]
    fn test_structural_type_all_primitives_ok() {
        let src = "
            anyInt: 'i64
            anyFloat: 'f64
            anyBool: 'bool
            main: 0
        ";
        assert!(check(src).is_ok());
    }

    #[test]
    fn test_structural_type_array_ok() {
        assert!(check("anyIntArray: '[i64]\nmain: 0").is_ok());
    }

    #[test]
    fn test_structural_type_nested_array_ok() {
        assert!(check("twoDim: '[[i64]]\nmain: 0").is_ok());
    }

    #[test]
    fn test_structural_type_map_ok() {
        assert!(check("anyMap: '{k1: bool, k2: i64}\nmain: 0").is_ok());
    }

    #[test]
    fn test_structural_type_function_ok() {
        assert!(check("anyFn: '(i64 | bool)\nmain: 0").is_ok());
    }

    #[test]
    fn test_structural_type_wildcard_function_ok() {
        assert!(check("discard: '(_|_)\nmain: 0").is_ok());
    }

    #[test]
    fn test_call_structural_type_function_ok() {
        // `(anyInt 42)` — calling a structural type function is an identity call
        let src = "
            anyInt: 'i64
            main: (anyInt 42)
        ";
        assert!(check(src).is_ok());
    }

    #[test]
    fn test_named_type_annotation_ok() {
        // `labelUsage anyFloat: 42` — function with named type annotation
        let src = "
            anyFloat: 'f64
            labelUsage anyFloat: 42
            main: 0
        ";
        assert!(check(src).is_ok());
    }

    #[test]
    fn test_named_type_annotation_undefined_error() {
        // Named annotation referencing a non-existent type should be an error
        let src = "
            labelUsage unknownType: 42
            main: 0
        ";
        let msg = check_err(src);
        assert!(
            msg.contains("undefined type") || msg.contains("unknownType"),
            "expected undefined-type error, got: {msg}"
        );
    }

    #[test]
    fn test_inline_function_type_annotation_ok() {
        // `id '(i64 | i64): x` — annotated identity function
        let src = "
            id '(i64 | i64): x
            main: 0
        ";
        assert!(check(src).is_ok());
    }

    #[test]
    fn test_inline_type_annotation_return_type_mismatch() {
        // The body returns a string but the annotation says i64 output.
        // (Strings are `str`, i64 is `int`, so this should be a type error.)
        let src = r#"
            f '(i64 | i64): "hello"
            main: 0
        "#;
        let msg = check_err(src);
        assert!(
            msg.contains("mismatch"),
            "expected type mismatch from annotation, got: {msg}"
        );
    }

    // ── generic structural types ─────────────────────────────────────────────

    #[test]
    fn test_generic_wildcard_function_ok() {
        // `<T _> (T | T)` — identity for any type
        assert!(check("genericId: '<T _> (T | T)\nmain: 0").is_ok());
    }

    #[test]
    fn test_generic_constrained_function_ok() {
        // `<T i64> (T | T)` — T must be i64
        assert!(check("intId: '<T i64> (T | T)\nmain: 0").is_ok());
    }

    #[test]
    fn test_generic_map_ok() {
        // `<T _> {k1: T, k2: T}` — both fields share type T
        assert!(check("pairOfSame: '<T _> {k1: T, k2: T}\nmain: 0").is_ok());
    }

    #[test]
    fn test_generic_multiple_params_ok() {
        assert!(check("multi: '<T i64, U _> {k1: T, k2: U}\nmain: 0").is_ok());
    }

    #[test]
    fn test_generic_nested_constraint_ok() {
        // `<T i64, U <V _>{k1: V}>` — nested generic in constraint
        assert!(
            check("nested: '<T i64, U <V _>{k1: V}> {k1: T, k2: U, k3: string}\nmain: 0").is_ok()
        );
    }

    #[test]
    fn test_generic_call_constrained_ok() {
        // Calling an i64-constrained generic identity with an int is fine.
        let src = "
            intId: '<T i64> (T | T)
            main: (intId 42)
        ";
        assert!(check(src).is_ok());
    }

    #[test]
    fn test_generic_call_constrained_type_mismatch() {
        // Calling an i64-constrained identity with a string is a type error.
        let src = r#"
            intId: '<T i64> (T | T)
            main: (intId "hello")
        "#;
        let msg = check_err(src);
        assert!(
            msg.contains("mismatch"),
            "expected type mismatch, got: {msg}"
        );
    }

    #[test]
    fn test_generic_call_wildcard_int_ok() {
        // Wildcard generic accepts any type, including int.
        let src = "
            genericId: '<T _> (T | T)
            main: (genericId 0)
        ";
        assert!(check(src).is_ok());
    }

    // ── nominal types ────────────────────────────────────────────────────────

    #[test]
    fn test_nominal_type_decl_ok() {
        assert!(check("specialInt: *i64\nmain: 0").is_ok());
    }

    #[test]
    fn test_nominal_type_named_annotation_literal_ok() {
        // `z specialInt: 1` — annotation with bare literal body is valid.
        let src = "
            specialInt: *i64
            z specialInt: 1
            main: 0
        ";
        assert!(check(src).is_ok());
    }

    #[test]
    fn test_nominal_type_constructor_call_ok() {
        // `a: (specialInt 1)` — explicit constructor call is valid anywhere.
        let src = "
            specialInt: *i64
            a: (specialInt 1)
            main: 0
        ";
        assert!(check(src).is_ok());
    }

    #[test]
    fn test_nominal_generic_ok() {
        assert!(check("b: *<T _> T\nmain: 0").is_ok());
    }

    #[test]
    fn test_nominal_generic_structural_specialization_ok() {
        // `c: '<T i64> b` — structural annotation specializing nominal generic.
        let src = "
            b: *<T _> T
            c: '<T i64> b
            main: 0
        ";
        assert!(check(src).is_ok());
    }

    #[test]
    fn test_nominal_generic_nominal_specialization_ok() {
        // `d: *<T i64> b` — nominal annotation specializing nominal generic.
        let src = "
            b: *<T _> T
            d: *<T i64> b
            main: 0
        ";
        assert!(check(src).is_ok());
    }

    #[test]
    fn test_nominal_all_success_cases_ok() {
        // All success cases from the spec in a single program.
        let src = "
            specialInt: *i64
            z specialInt: 1
            a: (specialInt 1)
            b: *<T _> T
            c: '<T i64> b
            d: *<T i64> b
            main: (specialInt 0)
        ";
        assert!(check(src).is_ok());
    }

    #[test]
    fn test_nominal_inline_annotation_parse_error() {
        // `uncallable *i64: 1` — `*typeExpr` as annotation is a parse error.
        let result = crate::lisp::parser::parse("uncallable *i64: 1\nmain: 0");
        assert!(result.is_err(), "expected parse error for `*` annotation");
    }

    #[test]
    fn test_nominal_inline_call_type_error() {
        // `uncallable2: (*i64 1)` — `*typeExpr` inline in call is a type error.
        let msg = check_err("uncallable2: (*i64 1)\nmain: 0");
        assert!(
            msg.contains("cannot call") || msg.contains("NominalType"),
            "expected cannot-call error, got: {msg}"
        );
    }

    #[test]
    fn test_nominal_annotation_grouped_expr_error() {
        // `noNominalClaim specialInt: (1)` — grouped expression (List) in body
        // that does not call the nominal constructor is a type error.
        let src = "
            specialInt: *i64
            noNominalClaim specialInt: (1)
            main: 0
        ";
        let msg = check_err(src);
        assert!(
            msg.contains("nominal type") || msg.contains("constructor"),
            "expected nominal-constructor error, got: {msg}"
        );
    }

    #[test]
    fn test_nominal_annotation_with_constructor_ok() {
        // `noNominalClaim specialInt: (specialInt 1)` — correct form.
        let src = "
            specialInt: *i64
            noNominalClaim specialInt: (specialInt 1)
            main: 0
        ";
        assert!(check(src).is_ok());
    }

    #[test]
    fn test_nominal_annotation_symbol_body_ok() {
        // Body is a plain symbol (the implicit parameter `x`) — allowed.
        let src = "
            specialInt: *i64
            passThrough specialInt: x
            main: 0
        ";
        assert!(check(src).is_ok());
    }
}
