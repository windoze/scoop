//! M9 tests: the `UInt` basic type and the core GC facilities
//! (milestone9 DESIGN.md section 1) — `UInt` resolution, equality and
//! arithmetic, the M12 `T : ref` reference-kind constraint of the
//! `pin` / `unpin` / `getGcHandle` / `releaseGcHandle` intrinsics,
//! the `gcCollect` / `gcStats` hooks,
//! and `PinnedPtr` / `GcHandle` construction and field access.

use super::*;

/// The `(name, type name)` pairs of `main`'s body locals, in
/// allocation order.
fn main_local_types(module: &hir::Module) -> Vec<(String, String)> {
    let main = &module.functions[module.entry];
    let hir::FunctionKind::User(body) = &main.kind else {
        panic!("main is a user function")
    };
    body.locals
        .iter()
        .map(|(_, local)| (local.name.clone(), hir::type_name(module, local.ty)))
        .collect()
}

/// The type name of one of `main`'s locals.
fn local_ty(module: &hir::Module, name: &str) -> String {
    main_local_types(module)
        .into_iter()
        .find(|(local, _)| local == name)
        .unwrap_or_else(|| panic!("no local `{name}`"))
        .1
}

mod gc;
mod gc_constraints;
mod generic_structs;
mod handles;
mod interfaces;
mod uint;
