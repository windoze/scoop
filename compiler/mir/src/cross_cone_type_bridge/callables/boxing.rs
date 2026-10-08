use super::*;
use crate::{CallableOwner, CallableSignatureSubject, ConeMirInput, FunctionId, InterfaceAdjust};
use std::collections::BTreeMap;

mod binding;

impl CanonicalMirCallableBindingsV1 {
    /// Projects interface receiver adapters for locally exported concrete owners.
    pub fn from_interface_adjusts(
        input: &ConeMirInput,
        local_types: &CanonicalParamFreeMirTypeExportsV1,
        identities: &ValidatedIdentityGraph,
        types: &dyn MirTypeBridgeTypeLookupV1,
        source_callables: &dyn MirTypeBridgeCallableLookupV1,
    ) -> Result<Self, MirBoxingCallableProductionError> {
        let mut roots = BTreeMap::new();
        for root in input.materialization().callable_roots() {
            roots.insert(root.function(), root.subject());
        }
        let adjusts = &input.module().meta.interface_adjusts;
        let mut records = Vec::new();

        scoop_wire::allocation::try_reserve(&mut records, adjusts.len(), &WirePath::root())?;
        for adjust in adjusts {
            let owner = match adjust.identity().callable_record().key() {
                GeneratedCallableKey::BoxingAdjust { payload, .. } => *payload,
                GeneratedCallableKey::DispatchAdjust { implementor, .. } => *implementor,
                _ => return Err(Error::InvalidAdjust(adjust.function())),
            };
            if local_types.get(owner).is_none()
                && !matches!(
                    identities
                        .canonical_key::<_, ExactTypeKey>(owner)
                        .map_err(MirCallableBridgeError::from)?
                        .as_ref(),
                    ExactTypeKey::Tuple(_) | ExactTypeKey::RawPointer(_)
                )
            {
                continue;
            }
            records.push(binding::project(
                input,
                identities,
                types,
                source_callables,
                &roots,
                adjust,
            )?);
        }

        Ok(Self::try_new(records)?)
    }
}

type Error = MirBoxingCallableProductionError;

#[derive(Debug)]
pub enum MirBoxingCallableProductionError {
    Resource(WireError),
    Bridge(MirCallableBridgeError),
    InvalidAdjust(FunctionId),
    MissingRoot(FunctionId),
    InvalidTarget(CallableOwner),
    MissingTargetBinding(CallableDefinitionOwner),
    TargetMismatch(CallableDefinitionOwner),
    MissingSignature(CallableSignatureSubject),
}
impl From<WireError> for Error {
    fn from(error: WireError) -> Self {
        Self::Resource(error)
    }
}
impl From<MirCallableBridgeError> for Error {
    fn from(error: MirCallableBridgeError) -> Self {
        Self::Bridge(error)
    }
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "cannot produce boxing MIR callable bindings: {self:?}")
    }
}
impl std::error::Error for Error {}
