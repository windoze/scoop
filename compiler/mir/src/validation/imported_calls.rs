//! Validation shared by the disjoint trusted-core and ordinary-dependency
//! callable arenas.

use crate::{Call, CallKind, Callee, MirValidationErrorKind, Module};

pub(super) fn validate_imported_call(
    module: &Module,
    call: &Call,
) -> Result<(), MirValidationErrorKind> {
    match call.target.callee {
        Callee::CoreExternal(callable) => {
            if !matches!(call.target.kind, CallKind::Direct) {
                return Err(MirValidationErrorKind::ImportedCoreCallableRequiresDirect);
            }
            let index = callable.into_raw().into_u32() as usize;
            if index >= module.meta.imported_core_callables.len() {
                return Err(
                    MirValidationErrorKind::InvalidImportedCoreCallableReference { callable },
                );
            }
        }
        Callee::DependencyStrong(callable) => {
            if !matches!(call.target.kind, CallKind::Direct) {
                return Err(MirValidationErrorKind::ImportedDependencyCallableRequiresDirect);
            }
            let index = callable.into_raw().into_u32() as usize;
            if index >= module.meta.imported_dependency_callables.len() {
                return Err(
                    MirValidationErrorKind::InvalidImportedDependencyCallableReference { callable },
                );
            }
        }
        Callee::User(_)
        | Callee::Monomorphized(_)
        | Callee::Extern(_)
        | Callee::CoroutineSuspend { .. }
        | Callee::Closure(_)
        | Callee::FunctionBridge(_)
        | Callee::Runtime(_) => {}
    }
    Ok(())
}
