use std::fmt;

use scoop_identity::{DigestKind, DigestNodeId, PersistentStaticStorageId};
use scoop_lir::{
    StaticStorageScanKindV1, StrongStaticStorageInitialStatePlanV1,
    StrongStaticStorageRegistrationPlanV1,
};
use scoop_wire::{
    HashError, RuntimeEncode, RuntimeEncodeError, RuntimeEncoder, domain_separated_runtime_hash,
};

use super::VerifiedStrongStaticStorageShapeFingerprintSetV1;
use crate::link_object::callable_registrations::object_definition::CanonicalDigestInputV1;
use crate::link_object::{
    LayoutFingerprintV1, ObjectDefinitionFingerprintV1, ScanFingerprintV1,
    StrongRegistrationFingerprintV1,
};

const STRONG_REGISTRATION_DOMAIN: &str = "scoop-strong-registration-v1";
const STATIC_STORAGE_RECORD_KIND: u32 = 1;
const STRONG_LINKAGE: u32 = 1;
const OWN_STORAGE_ATOM_ROLE: u32 = 1;
const EMPTY_SCAN_CHOICE: u32 = 0;
const SCAN_PROGRAM_CHOICE: u32 = 1;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VerifiedStrongStaticStorageFingerprintV1 {
    storage: PersistentStaticStorageId,
    registration_object_node: DigestNodeId,
    registration_object: ObjectDefinitionFingerprintV1,
    storage_definition_node: DigestNodeId,
    storage_definition: ObjectDefinitionFingerprintV1,
    layout_node: DigestNodeId,
    layout: LayoutFingerprintV1,
    scan_node: DigestNodeId,
    scan: ScanFingerprintV1,
    registration_node: DigestNodeId,
    registration: StrongRegistrationFingerprintV1,
}

impl VerifiedStrongStaticStorageFingerprintV1 {
    pub const fn storage(self) -> PersistentStaticStorageId {
        self.storage
    }

    pub const fn registration_object_node(self) -> DigestNodeId {
        self.registration_object_node
    }

    pub const fn registration_object(self) -> ObjectDefinitionFingerprintV1 {
        self.registration_object
    }

    pub const fn storage_definition_node(self) -> DigestNodeId {
        self.storage_definition_node
    }

    pub const fn storage_definition(self) -> ObjectDefinitionFingerprintV1 {
        self.storage_definition
    }

    pub const fn layout_node(self) -> DigestNodeId {
        self.layout_node
    }

    pub const fn layout(self) -> LayoutFingerprintV1 {
        self.layout
    }

    pub const fn scan_node(self) -> DigestNodeId {
        self.scan_node
    }

    pub const fn scan(self) -> ScanFingerprintV1 {
        self.scan
    }

    pub const fn registration_node(self) -> DigestNodeId {
        self.registration_node
    }

    pub const fn registration(self) -> StrongRegistrationFingerprintV1 {
        self.registration
    }
}

/// Canonical static-storage registration fingerprints derived from the exact
/// registration/storage object leaves and their layout/scan semantic leaves.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedStrongStaticStorageFingerprintSetV1 {
    shapes: VerifiedStrongStaticStorageShapeFingerprintSetV1,
    fingerprints: Vec<VerifiedStrongStaticStorageFingerprintV1>,
}

impl VerifiedStrongStaticStorageFingerprintSetV1 {
    pub const fn producer(&self) -> scoop_identity::ConeIdentity {
        self.shapes.producer()
    }

    pub const fn shapes(&self) -> &VerifiedStrongStaticStorageShapeFingerprintSetV1 {
        &self.shapes
    }

    pub fn fingerprints(&self) -> &[VerifiedStrongStaticStorageFingerprintV1] {
        &self.fingerprints
    }
}

pub fn compute_strong_static_storage_fingerprints_v1(
    shapes: VerifiedStrongStaticStorageShapeFingerprintSetV1,
) -> Result<VerifiedStrongStaticStorageFingerprintSetV1, StrongStaticStorageFingerprintError> {
    let storage_definitions = shapes.storage_definitions();
    let registration_objects = storage_definitions.registration_objects();
    let registrations = registration_objects.registrations();
    let verified = registrations.registrations();
    let planned = registrations.plan().registrations();
    if verified.len() != planned.len()
        || verified.len() != registration_objects.fingerprints().len()
        || verified.len() != storage_definitions.fingerprints().len()
        || verified.len() != shapes.fingerprints().len()
    {
        return Err(StrongStaticStorageFingerprintError::ProofCoverageMismatch);
    }

    let mut fingerprints = Vec::with_capacity(verified.len());
    for ((((verified, plan), registration_object), storage_definition), shape) in verified
        .iter()
        .zip(planned)
        .zip(registration_objects.fingerprints())
        .zip(storage_definitions.fingerprints())
        .zip(shapes.fingerprints())
    {
        let storage = plan.semantic().storage();
        if verified.storage() != storage
            || registration_object.storage() != storage
            || registration_object.node() != plan.registration_object_node()
        {
            return Err(
                StrongStaticStorageFingerprintError::RegistrationObjectMismatch { storage },
            );
        }
        if storage_definition.storage() != storage
            || storage_definition.node() != plan.storage_definition_node()
        {
            return Err(StrongStaticStorageFingerprintError::StorageDefinitionMismatch { storage });
        }
        if shape.storage() != storage
            || shape.layout() != plan.semantic().layout()
            || shape.layout_node() != plan.layout_fingerprint_node()
        {
            return Err(StrongStaticStorageFingerprintError::LayoutMismatch { storage });
        }
        if shape.scan() != plan.semantic().scan()
            || shape.scan_node() != plan.scan_fingerprint_node()
        {
            return Err(StrongStaticStorageFingerprintError::ScanMismatch { storage });
        }
        let registration = strong_static_storage_fingerprint(
            plan,
            registration_object.node(),
            registration_object.fingerprint(),
            storage_definition.node(),
            storage_definition.fingerprint(),
            shape.layout_node(),
            shape.layout_fingerprint(),
            shape.scan_node(),
            shape.scan_fingerprint(),
        )
        .map_err(|source| StrongStaticStorageFingerprintError::Hash { storage, source })?;
        fingerprints.push(VerifiedStrongStaticStorageFingerprintV1 {
            storage,
            registration_object_node: registration_object.node(),
            registration_object: registration_object.fingerprint(),
            storage_definition_node: storage_definition.node(),
            storage_definition: storage_definition.fingerprint(),
            layout_node: shape.layout_node(),
            layout: shape.layout_fingerprint(),
            scan_node: shape.scan_node(),
            scan: shape.scan_fingerprint(),
            registration_node: plan.registration_fingerprint_node(),
            registration,
        });
    }

    Ok(VerifiedStrongStaticStorageFingerprintSetV1 {
        shapes,
        fingerprints,
    })
}

#[allow(clippy::too_many_arguments)]
fn strong_static_storage_fingerprint(
    plan: &StrongStaticStorageRegistrationPlanV1,
    registration_object_node: DigestNodeId,
    registration_object: ObjectDefinitionFingerprintV1,
    storage_definition_node: DigestNodeId,
    storage_definition: ObjectDefinitionFingerprintV1,
    layout_node: DigestNodeId,
    layout: LayoutFingerprintV1,
    scan_node: DigestNodeId,
    scan: ScanFingerprintV1,
) -> Result<StrongRegistrationFingerprintV1, HashError> {
    let mut direct_inputs = [
        CanonicalDigestInputV1 {
            kind: DigestKind::ObjectDefinition,
            node: registration_object_node,
            digest: *registration_object.as_array(),
        },
        CanonicalDigestInputV1 {
            kind: DigestKind::ObjectDefinition,
            node: storage_definition_node,
            digest: *storage_definition.as_array(),
        },
        CanonicalDigestInputV1 {
            kind: DigestKind::Layout,
            node: layout_node,
            digest: *layout.as_array(),
        },
        CanonicalDigestInputV1 {
            kind: DigestKind::Scan,
            node: scan_node,
            digest: *scan.as_array(),
        },
    ];
    direct_inputs.sort_unstable_by_key(|input| (input.kind.tag(), input.node));
    domain_separated_runtime_hash(
        STRONG_REGISTRATION_DOMAIN,
        &StrongStaticStorageFingerprintInputV1 {
            plan,
            layout,
            scan,
            direct_inputs,
        },
    )
    .map(|digest| StrongRegistrationFingerprintV1::from_array(*digest.as_array()))
}

struct StrongStaticStorageFingerprintInputV1<'a> {
    plan: &'a StrongStaticStorageRegistrationPlanV1,
    layout: LayoutFingerprintV1,
    scan: ScanFingerprintV1,
    direct_inputs: [CanonicalDigestInputV1; 4],
}

impl RuntimeEncode for StrongStaticStorageFingerprintInputV1<'_> {
    fn runtime_encode(&self, encoder: &mut RuntimeEncoder) -> Result<(), RuntimeEncodeError> {
        let semantic = self.plan.semantic();
        encoder.u32(STATIC_STORAGE_RECORD_KIND)?;
        encoder.u32(STRONG_LINKAGE)?;
        encoder.fixed(semantic.storage().as_array())?;
        encoder.fixed(&[0; 32])?;
        encoder.fixed(&[0; 32])?;
        encoder.fixed(&[0; 32])?;
        encoder.u32(semantic.scan_kind().tag())?;
        encoder.u32(OWN_STORAGE_ATOM_ROLE)?;
        encoder.u64(semantic.byte_size())?;
        encoder.u64(semantic.allocation_extent())?;
        encoder.u64(semantic.required_alignment())?;
        match semantic.scan_kind() {
            StaticStorageScanKindV1::None => encoder.u32(EMPTY_SCAN_CHOICE)?,
            StaticStorageScanKindV1::Recursive => {
                encoder.u32(SCAN_PROGRAM_CHOICE)?;
                self.scan.runtime_encode(encoder)?;
            }
        }
        self.scan.runtime_encode(encoder)?;
        self.layout.runtime_encode(encoder)?;
        match semantic.initial_state() {
            StrongStaticStorageInitialStatePlanV1::ZeroedForRuntimeUnit => encoder.u32(1)?,
            StrongStaticStorageInitialStatePlanV1::EncodedStaticValue {
                initial_template,
                immortal_relocations,
            } => {
                encoder.u32(2)?;
                encoder.byte_span(initial_template)?;
                encoder.sequence_length(immortal_relocations.len())?;
                for relocation in immortal_relocations {
                    encoder.u64(relocation.pointer_offset())?;
                    encoder.fixed(relocation.target().as_array())?;
                }
            }
        }
        encoder.sequence_length(self.direct_inputs.len())?;
        for input in self.direct_inputs {
            input.runtime_encode(encoder)?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StrongStaticStorageFingerprintError {
    ProofCoverageMismatch,
    RegistrationObjectMismatch {
        storage: PersistentStaticStorageId,
    },
    StorageDefinitionMismatch {
        storage: PersistentStaticStorageId,
    },
    LayoutMismatch {
        storage: PersistentStaticStorageId,
    },
    ScanMismatch {
        storage: PersistentStaticStorageId,
    },
    Hash {
        storage: PersistentStaticStorageId,
        source: HashError,
    },
}

impl fmt::Display for StrongStaticStorageFingerprintError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "failed to compute strong static-storage fingerprints: {self:?}"
        )
    }
}

impl std::error::Error for StrongStaticStorageFingerprintError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Hash { source, .. } => Some(source),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests;
