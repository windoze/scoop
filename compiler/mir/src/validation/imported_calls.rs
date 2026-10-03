//! Reference and receiver checks for the shared external-use arena.

use crate::{Call, CallKind, Callee, MirValidationErrorKind, Module};

pub(super) fn validate_imported_call(
    module: &Module,
    call: &Call,
) -> Result<(), MirValidationErrorKind> {
    match call.target.callee {
        Callee::External(callable) => {
            let index = callable.into_raw().into_u32() as usize;
            if index >= module.meta.external_callables.len() {
                return Err(MirValidationErrorKind::InvalidExternalCallableReference { callable });
            }
            let invalid = |reason| MirValidationErrorKind::InvalidExternalDispatch { reason };
            match call.target.kind {
                CallKind::Direct => {}
                CallKind::Virtual { .. } => {
                    let Some(crate::Expr {
                        ty: crate::Type::Class(class),
                        ..
                    }) = call.args.first()
                    else {
                        return Err(invalid("external virtual call requires a class receiver"));
                    };
                    if class.into_raw().into_u32() as usize >= module.classes.len() {
                        return Err(invalid("external virtual receiver class is missing"));
                    }
                }
                CallKind::Interface { interface, slot } => {
                    let Some((_, declaration)) =
                        module.interfaces.iter().find(|(id, _)| *id == interface)
                    else {
                        return Err(invalid("external interface call names a missing interface"));
                    };
                    if slot as usize >= declaration.methods.len()
                        || call.args.first().map(|argument| &argument.ty)
                            != Some(&crate::Type::Interface(interface))
                    {
                        return Err(invalid(
                            "external interface call has an invalid receiver or slot",
                        ));
                    }
                }
                CallKind::Closure { .. } | CallKind::FunctionBridge { .. } => {
                    return Err(invalid("external declarations do not use closure dispatch"));
                }
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
