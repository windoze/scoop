//! Replay shared publication records from the same checked callable layouts.

use scoop_identity::{ExactCallableSignature, ScoopAbiArgument, StrongCallableDefinitionOwner};
use scoop_wire::{BudgetMeter, WireError, WirePath};

use super::{CallableAbiBuildError, CallableAbiRecordV1};
use crate::{
    CallableAbiLayoutInputsV1, ExactCallableAbiError, ExactCallableAbiExportV1,
    ExactCallableProtocolV1, ExternalCallableRootPlan, LirTargetProfile, OdrFreeLirFoundation,
};

impl CallableAbiRecordV1 {
    pub fn replay(
        target_profile: LirTargetProfile,
        target: StrongCallableDefinitionOwner,
        signature: ExactCallableSignature,
        protocol: ExactCallableProtocolV1,
        layouts: CallableAbiLayoutInputsV1<'_>,
        foundation: &OdrFreeLirFoundation,
        meter: &mut BudgetMeter,
    ) -> Result<Self, CallableAbiReplayError> {
        let exact = ExactCallableAbiExportV1::replay(
            target_profile,
            target,
            signature,
            protocol,
            layouts,
            foundation,
            meter,
        )?;
        let signature = exact.canonical_signature();
        let parameters = signature.signature().parameters().len() as u64;
        let arguments = signature.arguments().len() as u64;
        let path = WirePath::root();
        meter.charge_work(parameters.saturating_add(arguments), &path)?;
        meter.charge_collection_slots(parameters.saturating_add(arguments), &path)?;
        meter.charge_owned_bytes(
            parameters
                .saturating_mul(std::mem::size_of::<scoop_identity::PersistentExactTypeId>() as u64)
                .saturating_add(
                    arguments.saturating_mul(std::mem::size_of::<ScoopAbiArgument>() as u64),
                ),
            &path,
        )?;
        let roots = match protocol {
            ExactCallableProtocolV1::OrdinaryManaged => ExternalCallableRootPlan::ManagedStatepoint,
            ExactCallableProtocolV1::OrdinaryNoGc => ExternalCallableRootPlan::NoGc,
        };
        Ok(Self::new(
            foundation.producer(),
            target,
            signature.clone(),
            exact.calling_convention(),
            roots,
        )?)
    }
}

#[derive(Debug)]
pub enum CallableAbiReplayError {
    Exact(ExactCallableAbiError),
    Build(CallableAbiBuildError),
    Resource(WireError),
}

macro_rules! from_error {
    ($source:ty, $variant:ident) => {
        impl From<$source> for CallableAbiReplayError {
            fn from(source: $source) -> Self {
                Self::$variant(source)
            }
        }
    };
}
from_error!(ExactCallableAbiError, Exact);
from_error!(CallableAbiBuildError, Build);
from_error!(WireError, Resource);

impl std::fmt::Display for CallableAbiReplayError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "shared callable ABI replay failed: {self:?}")
    }
}
impl std::error::Error for CallableAbiReplayError {}
