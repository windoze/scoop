//! M7 tests: function overloading (docs/milestone7/DESIGN.md) —
//! declaration rules (1.1), the layered + most-specific resolution
//! algorithm (1.2), and the `print` / `println` migration to ordinary
//! core overloads (section 2).

use super::*;
use ast::ClassModifier::*;

/// The `FunctionId` of a top-level function by name and parameter
/// type names (overloads share a name).
fn top_level_fn(module: &hir::Module, name: &str, param_tys: &[&str]) -> hir::FunctionId {
    module
        .top_level
        .iter()
        .copied()
        .find(|&id| {
            let f = &module.functions[id];
            f.name == name
                && f.params.len() == param_tys.len()
                && f.params
                    .iter()
                    .zip(param_tys)
                    .all(|(p, want)| hir::type_name(module, p.ty) == *want)
        })
        .unwrap_or_else(|| panic!("no top-level function `{name}` with params {param_tys:?}"))
}

/// The `FunctionId` of a method of `owner` by short name and declared
/// (non-`this`) parameter type names.
fn method_fn(module: &hir::Module, owner: &str, name: &str, param_tys: &[&str]) -> hir::FunctionId {
    module
        .functions
        .iter()
        .find(|(_, f)| {
            f.name == format!("{owner}.{name}")
                && f.params.len() == param_tys.len() + 1
                && f.params[1..]
                    .iter()
                    .zip(param_tys)
                    .all(|(p, want)| hir::type_name(module, p.ty) == *want)
        })
        .map(|(id, _)| id)
        .unwrap_or_else(|| panic!("no method `{owner}.{name}` with params {param_tys:?}"))
}

fn body_of(module: &hir::Module, function: hir::FunctionId) -> &hir::Body {
    match &module.functions[function].kind {
        hir::FunctionKind::User(body) => body,
        _ => panic!("expected a user function"),
    }
}

/// The call inside `main`'s `index`-th statement (`f(...)` as a
/// statement, or `println(inner)` where `inner` is the interesting
/// call when `unnest` is set).
fn call_in_main(
    module: &hir::Module,
    index: usize,
    unnest: bool,
) -> (hir::FunctionId, &[hir::Expr]) {
    let body = body_of(module, module.entry);
    let hir::StatementKind::Expr(expr) = &body.statements[index].kind else {
        panic!("statement {index} is not an expression statement")
    };
    let hir::ExprKind::Call { callee, args } = &expr.kind else {
        panic!("statement {index} is not a call")
    };
    if unnest {
        // Keep this helper usable for older boxing-oriented overload cases as
        // well as the generic core output functions, whose arguments retain
        // their concrete types.
        let inner = match &args[0].kind {
            hir::ExprKind::Box(operand) => &operand.kind,
            kind => kind,
        };
        let hir::ExprKind::Call { callee, args } = inner else {
            panic!("the argument of statement {index} is not a call")
        };
        (module.callable_function(*callee), args)
    } else {
        (module.callable_function(*callee), args)
    }
}

fn has_instantiation(
    module: &hir::Module,
    function: hir::FunctionId,
    type_args: &[hir::TypeId],
) -> bool {
    module.instantiations.iter().any(|(_, resolved)| {
        module.generic_functions[resolved.generic].function == function
            && resolved.type_args == type_args
    })
}

fn has_method_application(
    module: &hir::Module,
    function: hir::FunctionId,
    owner_arguments: &[hir::TypeId],
) -> bool {
    module.method_applications.iter().any(|(_, application)| {
        if application.function != function {
            return false;
        }
        let arguments: &[hir::TypeId] = match application.owner {
            hir::MethodOwnerApplication::Class(owner) => {
                &module.class_applications[owner].arguments
            }
            hir::MethodOwnerApplication::Struct(owner) => {
                &module.struct_applications[owner].arguments
            }
            hir::MethodOwnerApplication::Enum(owner) => &module.enum_applications[owner].arguments,
            hir::MethodOwnerApplication::Interface(owner) => {
                &module.interface_applications[owner].arguments
            }
        };
        arguments == owner_arguments
    })
}

mod applicability;
mod core_members;
mod declarations;
mod generic_enum;
mod layering;
mod specificity;
