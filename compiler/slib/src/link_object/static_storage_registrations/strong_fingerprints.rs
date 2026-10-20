use std::fmt;

use scoop_identity::{DigestNodeId, PersistentStaticStorageId};
use scoop_lir::{
    StaticStorageScanKindV1, StrongStaticStorageInitialStatePlanV1,
    StrongStaticStorageRegistrationPlanV1,
};
use scoop_wire::{HashError, RuntimeEncode, RuntimeEncodeError, RuntimeEncoder};

use super::StaticStorageShapeFingerprintSetV1;
use crate::link_object::{
    LayoutFingerprintV1, OdrMemberAbiV1, RegistrationAbiV1, ScanFingerprintV1,
};

const STATIC_STORAGE_RECORD_KIND: u32 = 1;
const OWN_STORAGE_ATOM_ROLE: u32 = 1;
const EMPTY_SCAN_CHOICE: u32 = 0;
const SCAN_PROGRAM_CHOICE: u32 = 1;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StaticStorageFingerprintV1 {
    storage: PersistentStaticStorageId,

    layout_node: DigestNodeId,
    layout: LayoutFingerprintV1,
    scan_node: DigestNodeId,
    scan: ScanFingerprintV1,

    registration: RegistrationAbiV1,
}

impl StaticStorageFingerprintV1 {
    pub const fn storage(self) -> PersistentStaticStorageId {
        self.storage
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

    pub const fn registration(self) -> RegistrationAbiV1 {
        self.registration
    }
}

/// Layout/scan fingerprints and shared ABIs of static storages.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StaticStorageFingerprintSetV1 {
    shapes: StaticStorageShapeFingerprintSetV1,
    fingerprints: Vec<StaticStorageFingerprintV1>,
    odr_definitions: Vec<OdrMemberAbiV1>,
}

impl StaticStorageFingerprintSetV1 {
    pub fn odr_definitions(&self) -> &[OdrMemberAbiV1] {
        &self.odr_definitions
    }
    pub const fn producer(&self) -> scoop_identity::ConeIdentity {
        self.shapes.producer()
    }

    pub const fn shapes(&self) -> &StaticStorageShapeFingerprintSetV1 {
        &self.shapes
    }

    pub fn fingerprints(&self) -> &[StaticStorageFingerprintV1] {
        &self.fingerprints
    }
}

pub fn compute_strong_static_storage_fingerprints_v1(
    shapes: StaticStorageShapeFingerprintSetV1,
    canonical: &scoop_lir::CanonicalShapeAbisV1,
) -> Result<StaticStorageFingerprintSetV1, StrongStaticStorageFingerprintError> {
    let registrations = shapes.registrations();
    let planned = registrations.registrations();
    let mut fingerprints = Vec::with_capacity(planned.len());
    let mut odr_definitions = Vec::new();
    for (plan, shape) in planned.iter().zip(shapes.fingerprints()) {
        let storage = plan.semantic().storage();
        if let scoop_lir::RegistrationDefinitionOwner::Odr { group, .. } = plan.definition_owner() {
            let content = canonical
                .definitions()
                .iter()
                .find(|content| {
                    content.definition() == plan.storage_definition_plan()
                        && content.group() == group
                        && content.role() == scoop_identity::OdrMemberRole::StaticStorage
                })
                .ok_or(
                    StrongStaticStorageFingerprintError::StorageDefinitionMismatch { storage },
                )?;
            odr_definitions.push(super::super::odr_member_fingerprints::shape_definition(
                *content,
            ));
        }
        let registration = RegistrationAbiV1::from_owner(
            plan.definition_owner(),
            scoop_lir::RegistrationTableV1::StaticStorage,
        )
        .map_err(|source| StrongStaticStorageFingerprintError::Hash { storage, source })?;
        fingerprints.push(StaticStorageFingerprintV1 {
            storage,

            layout_node: shape.layout_node(),
            layout: shape.layout_fingerprint(),
            scan_node: shape.scan_node(),
            scan: shape.scan_fingerprint(),

            registration,
        });
    }

    Ok(StaticStorageFingerprintSetV1 {
        shapes,
        fingerprints,
        odr_definitions,
    })
}

pub(in crate::link_object) fn runtime_encode_strong_static_storage_record_v1(
    encoder: &mut RuntimeEncoder,
    plan: &StrongStaticStorageRegistrationPlanV1,
    layout: LayoutFingerprintV1,
    scan: ScanFingerprintV1,
) -> Result<(), RuntimeEncodeError> {
    let semantic = plan.semantic();
    encoder.u32(STATIC_STORAGE_RECORD_KIND)?;
    super::super::registration_identity::runtime_encode_registration_identity(
        encoder,
        semantic.storage().as_array(),
        plan.definition_owner(),
    )?;
    encoder.u32(semantic.scan_kind().tag())?;
    encoder.u32(OWN_STORAGE_ATOM_ROLE)?;
    encoder.u64(semantic.byte_size())?;
    encoder.u64(semantic.allocation_extent())?;
    encoder.u64(semantic.required_alignment())?;
    match semantic.scan_kind() {
        StaticStorageScanKindV1::None => encoder.u32(EMPTY_SCAN_CHOICE)?,
        StaticStorageScanKindV1::Recursive => {
            encoder.u32(SCAN_PROGRAM_CHOICE)?;
            scan.runtime_encode(encoder)?;
        }
    }
    scan.runtime_encode(encoder)?;
    layout.runtime_encode(encoder)?;
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
    Ok(())
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StrongStaticStorageFingerprintError {
    StorageDefinitionMismatch {
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
