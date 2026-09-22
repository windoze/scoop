use super::*;
use crate::{
    BoxingAdjust, CallableOwner, CallableSignatureSubject, FunctionId, SingleConeStrongMirInput,
};
use std::collections::BTreeMap;

mod binding;

impl CanonicalMirCallableBindingsV1 {
    /// Projects actual boxed-value dispatch bodies for locally exported payloads.
    pub fn from_boxing_adjusts(
        input: &SingleConeStrongMirInput,
        local_types: &CanonicalParamFreeMirTypeExportsV1,
        identities: &ValidatedIdentityGraph,
        types: &dyn MirTypeBridgeTypeLookupV1,
        source_callables: &dyn MirTypeBridgeCallableLookupV1,
        meter: &mut BudgetMeter,
    ) -> Result<Self, MirBoxingCallableProductionError> {
        let mut roots = BTreeMap::new();
        for root in input.materialization().callable_roots() {
            work(search_cost(roots.len()), meter)?;
            meter.charge_collection_slots(1, &WirePath::root())?;
            meter.charge_owned_bytes(
                std::mem::size_of::<(FunctionId, CallableOwner)>() as u64,
                &WirePath::root(),
            )?;
            roots.insert(root.function(), root.implementation());
        }
        let adjusts = &input.module().meta.boxing_adjusts;
        let mut records = Vec::new();
        meter.charge_owned_bytes(
            (adjusts.len() as u64)
                .saturating_mul(std::mem::size_of::<ParamFreeMirCallableBindingV1>() as u64),
            &WirePath::root(),
        )?;
        meter.try_reserve_collection_slots(&mut records, adjusts.len(), &WirePath::root())?;
        for adjust in adjusts {
            meter.charge_nodes(1, &WirePath::root())?;
            work(search_cost(local_types.records().len()), meter)?;
            let GeneratedCallableKey::BoxingAdjust { payload, .. } =
                adjust.identity().callable_record().key()
            else {
                return Err(Error::InvalidAdjust(adjust.function()));
            };
            if local_types.get(*payload).is_none() {
                continue;
            }
            records.push(binding::project(
                input,
                identities,
                types,
                source_callables,
                &roots,
                adjust,
                meter,
            )?);
        }
        work(
            (records.len() as u64).saturating_mul(search_cost(records.len())),
            meter,
        )?;
        Ok(Self::try_new(records)?)
    }
}

type Error = MirBoxingCallableProductionError;

#[derive(Debug)]
pub enum MirBoxingCallableProductionError {
    Resource(WireError),
    Bridge(MirCallableBridgeError),
    InvalidAdjust(FunctionId),
    MissingStrongRoot(FunctionId),
    InvalidTarget(CallableOwner),
    MissingTargetBinding(StrongCallableDefinitionOwner),
    TargetMismatch(StrongCallableDefinitionOwner),
    MissingSignature(CallableOwner),
    SignatureMismatch(CallableOwner),
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

fn search_cost(count: usize) -> u64 {
    u64::from(count.checked_ilog2().unwrap_or(0)) + 1
}
fn work(count: u64, meter: &mut BudgetMeter) -> Result<(), Error> {
    Ok(meter.charge_work(count, &WirePath::root())?)
}
