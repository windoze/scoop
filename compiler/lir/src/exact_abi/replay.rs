use scoop_identity::{CallableBodyKey, PersistentExactTypeId};

use super::*;
use crate::ExternalStrongShapeSubjectV1;

pub(super) fn callable(
    target_profile: LirTargetProfile,
    target: StrongCallableDefinitionOwner,
    signature: CanonicalScoopAbiFunctionSignature,
    foundation: &OdrFreeLirFoundation,
) -> Result<ExactCallableAbiExportV1, ExactCallableAbiError> {
    let protocol = match signature.gc_effect() {
        GcEffect::Managed => ExactCallableProtocolV1::OrdinaryManaged,
        GcEffect::NoGc => ExactCallableProtocolV1::OrdinaryNoGc,
    };
    let body = PersistentCallableBodyId::from_key(&CallableBodyKey::strong(target))?;

    if !foundation
        .callable_bodies()
        .iter()
        .any(|record| record.id() == body)
    {
        return Err(ExactCallableAbiError::MissingCallableBody);
    }
    let physical = StrongShapeDefinitionRefV1::from_foundation(
        ExternalStrongShapeSubjectV1::Callable(target),
        foundation,
    )?;
    let definition = StrongShapeDefinitionV1::from_callable_definition(target, physical)?
        .ok_or(ExactCallableAbiError::DefinitionSubject)?;

    Ok(ExactCallableAbiExportV1(Arc::new(CallableAbiBodyV1 {
        target,
        target_profile,
        signature,
        protocol,
        physical,
        definition,
    })))
}

#[derive(Debug)]
pub enum ExactCallableAbiError {
    MissingValueLayout { exact: PersistentExactTypeId },
    DuplicateValueLayout { exact: PersistentExactTypeId },
    LayoutRole,
    MissingCallableBody,
    DefinitionSubject,
    Abi(scoop_identity::ScoopAbiError),
    Hash(scoop_wire::HashError),
    Definition(crate::StrongShapeDefinitionError),
    Resource(WireError),
}
macro_rules! from_error {
    ($source:ty, $variant:ident) => {
        impl From<$source> for ExactCallableAbiError {
            fn from(value: $source) -> Self {
                Self::$variant(value)
            }
        }
    };
}
from_error!(scoop_identity::ScoopAbiError, Abi);
from_error!(scoop_wire::HashError, Hash);
from_error!(crate::StrongShapeDefinitionError, Definition);
from_error!(WireError, Resource);
impl std::fmt::Display for ExactCallableAbiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "callable ABI replay failed: {self:?}")
    }
}
impl std::error::Error for ExactCallableAbiError {}
