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

/// The call inside `main`'s `index`-th source expression statement. When
/// `unnest` is set, the interesting inner call is found in its preceding
/// argument-evaluation temporary.
fn call_in_main(
    module: &hir::Module,
    index: usize,
    unnest: bool,
) -> (hir::FunctionId, &[hir::Expr]) {
    let body = body_of(module, module.entry);
    let statement_index = body
        .statements
        .iter()
        .enumerate()
        .filter(|(_, statement)| matches!(statement.kind, hir::StatementKind::Expr(_)))
        .nth(index)
        .map(|(statement_index, _)| statement_index)
        .unwrap_or_else(|| panic!("source expression statement {index} must exist"));
    let hir::StatementKind::Expr(expr) = &body.statements[statement_index].kind else {
        unreachable!()
    };
    let hir::ExprKind::Call { callee, args } = &expr.kind else {
        panic!("statement {index} is not a call")
    };
    if unnest {
        let (callee, args) = body.statements[..statement_index]
            .iter()
            .rev()
            .find_map(|statement| {
                let hir::StatementKind::ValDecl { init, .. } = &statement.kind else {
                    return None;
                };
                let inner = match &init.kind {
                    hir::ExprKind::Box(operand) => &operand.kind,
                    kind => kind,
                };
                let hir::ExprKind::Call { callee, args } = inner else {
                    return None;
                };
                Some((callee, args.as_slice()))
            })
            .unwrap_or_else(|| panic!("the argument of statement {index} is not a call"));
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
