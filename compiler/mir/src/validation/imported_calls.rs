//! Validation for direct calls through the shared external-use arena.

use crate::{Call, CallKind, Callee, MirValidationErrorKind, Module};

pub(super) fn validate_imported_call(
    module: &Module,
    call: &Call,
) -> Result<(), MirValidationErrorKind> {
    match call.target.callee {
        Callee::External(callable) => {
            if !matches!(call.target.kind, CallKind::Direct) {
                return Err(MirValidationErrorKind::ExternalCallableRequiresDirect);
            }
            let index = callable.into_raw().into_u32() as usize;
            if index >= module.meta.external_callables.len() {
                return Err(MirValidationErrorKind::InvalidExternalCallableReference { callable });
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
