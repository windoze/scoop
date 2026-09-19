//! Physical binding for v2 references, prior to layout/ABI selection checks.

use scoop_identity::{DecodedPersistentId, PersistentSymbolKey};
use scoop_wire::{BudgetMeter, WirePath};

use super::*;
use crate::{ExternalStrongShapeSubjectV1, StrongShapeDefinitionRefV1};

mod resolve;

/// A catalog of complete dependency TD/callable definitions. This does not
/// grant export, ABI, inheritance, or selected-closure authority. The artifact
/// reader must join these same definitions to its committed layout/ABI uses.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StrongTypeReferenceDefinitionsV2 {
    consumer: ConeIdentity,
    descriptors: Vec<StrongShapeDefinitionRefV1>,
    callables: Vec<StrongShapeDefinitionRefV1>,
}

impl StrongTypeReferenceDefinitionsV2 {
    pub fn new(
        consumer: ConeIdentity,
        definitions: &[StrongShapeDefinitionRefV1],
        meter: &mut BudgetMeter,
    ) -> Result<Self, StrongTypeReferenceResolutionErrorV2> {
        let path = WirePath::root();
        meter.charge_work((definitions.len() as u64).saturating_mul(64), &path)?;
        let mut descriptors = Vec::new();
        let mut callables = Vec::new();
        let descriptor_count = definitions
            .iter()
            .filter(|definition| {
                matches!(
                    definition.subject(),
                    ExternalStrongShapeSubjectV1::TypeDescriptor(_)
                )
            })
            .count();
        meter.try_reserve_collection_slots(&mut descriptors, descriptor_count, &path)?;
        meter.try_reserve_collection_slots(
            &mut callables,
            definitions.len() - descriptor_count,
            &path,
        )?;
        for definition in definitions {
            if definition.provider() == consumer {
                return Err(StrongTypeReferenceResolutionErrorV2::CurrentConeDefinition(
                    definition.subject(),
                ));
            }
            match definition.subject() {
                ExternalStrongShapeSubjectV1::TypeDescriptor(_) => descriptors.push(*definition),
                ExternalStrongShapeSubjectV1::Callable(_) => callables.push(*definition),
                subject => {
                    return Err(StrongTypeReferenceResolutionErrorV2::UnexpectedSubject(
                        subject,
                    ));
                }
            }
        }
        // Symbol identities are global. A second provider cannot supply a
        // different physical definition for the same descriptor or body.
        for entries in [&mut descriptors, &mut callables] {
            entries.sort_unstable_by_key(|definition| definition.symbol().key());
            if let Some(pair) = entries
                .windows(2)
                .find(|pair| pair[0].symbol() == pair[1].symbol())
            {
                return Err(StrongTypeReferenceResolutionErrorV2::DuplicateDefinition(
                    pair[0].symbol().key(),
                ));
            }
        }
        Ok(Self {
            consumer,
            descriptors,
            callables,
        })
    }

    pub const fn consumer(&self) -> ConeIdentity {
        self.consumer
    }

    pub fn descriptor_definitions(&self) -> &[StrongShapeDefinitionRefV1] {
        &self.descriptors
    }

    pub fn callable_definitions(&self) -> &[StrongShapeDefinitionRefV1] {
        &self.callables
    }

    fn check_producer(
        &self,
        actual: ConeIdentity,
    ) -> Result<(), StrongTypeReferenceResolutionErrorV2> {
        if actual == self.consumer {
            Ok(())
        } else {
            Err(StrongTypeReferenceResolutionErrorV2::ProducerMismatch {
                expected: self.consumer,
                actual,
            })
        }
    }
}

#[derive(Debug)]
pub enum StrongTypeReferenceResolutionErrorV2 {
    Resource(WireError),
    LocalDefinition(crate::StrongShapeDefinitionError),
    ProducerMismatch {
        expected: ConeIdentity,
        actual: ConeIdentity,
    },
    CurrentConeDefinition(ExternalStrongShapeSubjectV1),
    UnexpectedSubject(ExternalStrongShapeSubjectV1),
    DuplicateDefinition(PersistentSymbolKey),
    UnknownLocalDescriptor(DecodedPersistentId<PersistentExactTypeId>),
    UnknownCoreDescriptor(DecodedPersistentId<PersistentExactTypeId>),
    UnknownLocalCallable(DecodedPersistentId<PersistentCallableBodyId>),
    UnknownCoreCallable(DecodedPersistentId<PersistentCallableBodyId>),
    CoreDescriptorPartition(PersistentExactTypeId),
    CoreCallablePartition(PersistentCallableBodyId),
    LocalDescriptorPartition(PersistentExactTypeId),
    LocalCallablePartition(PersistentCallableBodyId),
    UnknownDependencyDescriptor {
        provider: DecodedPersistentId<ConeIdentity>,
        exact: DecodedPersistentId<PersistentExactTypeId>,
    },
    UnknownDependencyCallable {
        provider: DecodedPersistentId<ConeIdentity>,
        body: DecodedPersistentId<PersistentCallableBodyId>,
    },
}

impl From<WireError> for StrongTypeReferenceResolutionErrorV2 {
    fn from(error: WireError) -> Self {
        Self::Resource(error)
    }
}
impl std::fmt::Display for StrongTypeReferenceResolutionErrorV2 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "invalid v2 descriptor or dispatch reference: {self:?}")
    }
}
impl std::error::Error for StrongTypeReferenceResolutionErrorV2 {}
