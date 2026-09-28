//! Physical binding for v2 references, prior to layout/ABI selection checks.

use scoop_identity::{DecodedPersistentId, PersistentSymbolKey};
use scoop_wire::WirePath;

use super::*;
use crate::{ExternalStrongShapeSubjectV1, StrongShapeDefinitionRefV1};

mod resolve;

/// Complete dependency definitions and value layouts used by typed references.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StrongTypeReferenceDefinitionsV2 {
    consumer: ConeIdentity,
    descriptors: Vec<StrongShapeDefinitionRefV1>,
    callables: Vec<StrongShapeDefinitionRefV1>,
    layouts: Vec<crate::CanonicalExactLayoutExportsV1>,
}

impl StrongTypeReferenceDefinitionsV2 {
    pub fn new(
        consumer: ConeIdentity,
        definitions: &[StrongShapeDefinitionRefV1],
        layouts: &[&crate::CanonicalExactLayoutExportsV1],
    ) -> Result<Self, StrongTypeReferenceResolutionErrorV2> {
        let path = WirePath::root();

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
        scoop_wire::allocation::try_reserve(&mut descriptors, descriptor_count, &path)?;
        scoop_wire::allocation::try_reserve(
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
        // Keep the actual provider of each physical ODR definition. The
        // shared artifact merge compares member contents after this index
        // has bound references to their recorded providers.
        for entries in [&mut descriptors, &mut callables] {
            entries.sort_unstable_by_key(|definition| {
                (definition.symbol().key(), definition.provider())
            });
            if let Some(pair) = entries.windows(2).find(|pair| {
                pair[0].symbol().key() == pair[1].symbol().key()
                    && (pair[0].provider() == pair[1].provider()
                        || pair[0].symbol().linkage() != scoop_identity::LinkageClass::OdrWeak
                        || pair[1].symbol().linkage() != scoop_identity::LinkageClass::OdrWeak
                        || pair[0].definition() != pair[1].definition())
            }) {
                return Err(StrongTypeReferenceResolutionErrorV2::DuplicateDefinition(
                    pair[0].symbol().key(),
                ));
            }
        }
        Ok(Self {
            consumer,
            descriptors,
            callables,
            layouts: layouts.iter().map(|table| (*table).clone()).collect(),
        })
    }

    pub(crate) fn resolve_value_layout(
        &self,
        provider: DecodedPersistentId<ConeIdentity>,
        layout: DecodedPersistentId<scoop_identity::PersistentLayoutId>,
    ) -> Option<std::sync::Arc<crate::ExactValueLayoutV1>> {
        self.layouts
            .iter()
            .find(|table| table.provider().as_array() == provider.as_array())?
            .records()
            .iter()
            .find(|record| record.identity().layout().as_array() == layout.as_array())?
            .value_handle()
    }

    pub(crate) fn layouts(&self) -> &[crate::CanonicalExactLayoutExportsV1] {
        &self.layouts
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

    pub(crate) fn resolve_external_descriptor(
        &self,
        provider: DecodedPersistentId<ConeIdentity>,
        exact: DecodedPersistentId<PersistentExactTypeId>,
    ) -> Option<(ConeIdentity, PersistentExactTypeId)> {
        self.descriptors.iter().find_map(|definition| {
            let ExternalStrongShapeSubjectV1::TypeDescriptor(candidate) = definition.subject()
            else {
                return None;
            };
            (definition.provider().as_array() == provider.as_array()
                && candidate.as_array() == exact.as_array())
            .then_some((definition.provider(), candidate))
        })
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
    UnknownLocalCallable(DecodedPersistentId<PersistentCallableBodyId>),
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
