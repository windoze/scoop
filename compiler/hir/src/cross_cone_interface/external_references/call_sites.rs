//! Actual calls owned by the common external-reference record.

use scoop_identity::{ConcreteExpressionOrigin, PersistentExactTypeId};
use scoop_wire::{Encoder, WireEncode};

use crate::concrete::ExecutableExpressionPosition;

mod decode;
mod errors;
mod reason;
mod runtime;
mod source;
mod table;
#[cfg(test)]
mod tests;
pub use decode::{DecodedHirDependencyCallSiteV1, HirDependencyCallSiteResolver};
pub use errors::{HirDependencyCallSiteBuildError, HirDependencyCallSiteResolutionError};
use reason::DecodedHirDependencyCallReasonV1;
pub use reason::HirDependencyCallReasonV1;
pub use runtime::HirRuntimeConstructorError;
pub use source::HirDependencyCallSignatureError;
pub use table::{CanonicalHirDependencyCallSitesV1, DecodedCanonicalHirDependencyCallSitesV1};
#[cfg(test)]
pub(super) use tests::support::Fixture;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirDependencyCallSiteV1 {
    position: ExecutableExpressionPosition,
    origin: ConcreteExpressionOrigin,
    arguments: Vec<PersistentExactTypeId>,
    result: PersistentExactTypeId,
    reason: HirDependencyCallReasonV1,
    receiver: crate::SourceCallReceiver<PersistentExactTypeId>,
}

impl HirDependencyCallSiteV1 {
    pub fn try_new(
        position: ExecutableExpressionPosition,
        origin: ConcreteExpressionOrigin,
        arguments: Vec<PersistentExactTypeId>,
        result: PersistentExactTypeId,
        witness_indices: Vec<u32>,
        receiver: crate::SourceCallReceiver<PersistentExactTypeId>,
    ) -> Result<Self, HirDependencyCallSiteBuildError> {
        Self::try_new_with_reason(
            position,
            origin,
            arguments,
            result,
            HirDependencyCallReasonV1::SourceBinding(witness_indices),
            receiver,
        )
    }

    pub fn try_new_with_reason(
        position: ExecutableExpressionPosition,
        origin: ConcreteExpressionOrigin,
        arguments: Vec<PersistentExactTypeId>,
        result: PersistentExactTypeId,
        reason: HirDependencyCallReasonV1,
        receiver: crate::SourceCallReceiver<PersistentExactTypeId>,
    ) -> Result<Self, HirDependencyCallSiteBuildError> {
        if receiver.has_receiver() {
            if matches!(reason, HirDependencyCallReasonV1::CastFailure { .. }) {
                return Err(HirDependencyCallSiteBuildError::RuntimeReceiver);
            }
            if arguments.is_empty() {
                return Err(HirDependencyCallSiteBuildError::MissingReceiverArgument);
            }
        }
        match &reason {
            HirDependencyCallReasonV1::SourceBinding(indices) => validate_witness_indices(indices),
            HirDependencyCallReasonV1::CastFailure { .. } if !arguments.is_empty() => {
                Err(HirDependencyCallSiteBuildError::RuntimeArguments)
            }
            HirDependencyCallReasonV1::CastFailure { .. }
            | HirDependencyCallReasonV1::SourceDeclaration => Ok(()),
        }?;
        Ok(Self {
            position,
            origin,
            arguments,
            result,
            reason,
            receiver,
        })
    }

    pub const fn position(&self) -> ExecutableExpressionPosition {
        self.position
    }

    pub const fn origin(&self) -> &ConcreteExpressionOrigin {
        &self.origin
    }

    pub fn arguments(&self) -> &[PersistentExactTypeId] {
        &self.arguments
    }

    pub const fn result(&self) -> PersistentExactTypeId {
        self.result
    }

    pub const fn receiver(&self) -> crate::SourceCallReceiver<PersistentExactTypeId> {
        self.receiver
    }

    pub fn witness_indices(&self) -> &[u32] {
        match &self.reason {
            HirDependencyCallReasonV1::SourceBinding(indices) => indices,
            HirDependencyCallReasonV1::CastFailure { .. }
            | HirDependencyCallReasonV1::SourceDeclaration => &[],
        }
    }

    pub const fn reason(&self) -> &HirDependencyCallReasonV1 {
        &self.reason
    }
}

impl WireEncode for HirDependencyCallSiteV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(7)?;
        encoder.field(1)?;
        self.position.root.encode(encoder)?;
        encoder.field(2)?;
        encoder.unsigned(u64::from(self.position.expression_index))?;
        encoder.field(3)?;
        self.origin.encode(encoder)?;
        encoder.field(4)?;
        encoder.array(self.arguments.len() as u64)?;
        for argument in &self.arguments {
            argument.encode(encoder)?;
        }
        encoder.field(5)?;
        self.result.encode(encoder)?;
        encoder.field(6)?;
        self.reason.encode(encoder)?;
        encoder.field(7)?;
        self.receiver.encode(encoder)
    }
}

fn encode_indices(
    values: &[u32],
    encoder: &mut Encoder,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.array(values.len() as u64)?;
    for value in values {
        encoder.unsigned(u64::from(*value))?;
    }
    Ok(())
}

fn validate_witness_indices(indices: &[u32]) -> Result<(), HirDependencyCallSiteBuildError> {
    if indices.is_empty() {
        return Err(HirDependencyCallSiteBuildError::EmptyWitnessIndices);
    }
    if let Some((index, _)) = indices
        .windows(2)
        .enumerate()
        .find(|(_, pair)| pair[0] >= pair[1])
    {
        return Err(HirDependencyCallSiteBuildError::WitnessIndexOrder { index: index + 1 });
    }
    Ok(())
}
