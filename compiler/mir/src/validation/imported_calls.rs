//! Reference and receiver checks for the shared external-use arena.

use crate::{
    Call, CallTarget, Callee, DispatchCallee, ExternalCallableUseId, MirValidationErrorKind,
    Module, Type,
};

pub(super) fn validate_imported_call(
    module: &Module,
    call: &Call,
) -> Result<(), MirValidationErrorKind> {
    let invalid = |reason| MirValidationErrorKind::InvalidExternalDispatch { reason };
    match &call.target {
        CallTarget::Direct(Callee::External(callable)) => validate_reference(module, *callable)?,
        CallTarget::Virtual {
            callee: DispatchCallee::External(callable),
            receiver,
            ..
        } => {
            validate_reference(module, *callable)?;
            let Type::Class(class) = receiver.ty else {
                return Err(invalid("external virtual call requires a class receiver"));
            };
            if class.into_raw().into_u32() as usize >= module.classes.len() {
                return Err(invalid("external virtual receiver class is missing"));
            }
        }
        CallTarget::Interface {
            callee: DispatchCallee::External(callable),
            interface,
            slot,
            receiver,
        } => {
            validate_reference(module, *callable)?;
            let Some((_, declaration)) = module.interfaces.iter().find(|(id, _)| id == interface)
            else {
                return Err(invalid("external interface call names a missing interface"));
            };
            if *slot as usize >= declaration.methods.len()
                || receiver.ty != Type::Interface(*interface)
            {
                return Err(invalid(
                    "external interface call has an invalid receiver or slot",
                ));
            }
        }
        _ => {}
    }
    Ok(())
}

fn validate_reference(
    module: &Module,
    callable: ExternalCallableUseId,
) -> Result<(), MirValidationErrorKind> {
    if callable.into_raw().into_u32() as usize >= module.meta.external_callables.len() {
        Err(MirValidationErrorKind::InvalidExternalCallableReference { callable })
    } else {
        Ok(())
    }
}
