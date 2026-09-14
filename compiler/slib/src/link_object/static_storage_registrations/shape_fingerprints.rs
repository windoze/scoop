use std::fmt;

use scoop_identity::{
    DigestNodeId, PersistentLayoutId, PersistentScanId, PersistentStaticStorageId,
};
use scoop_lir::RefScan;
use scoop_wire::{
    HashError, RuntimeEncode, RuntimeEncodeError, RuntimeEncoder, domain_separated_runtime_hash,
};

use super::VerifiedStrongStaticStorageDefinitionFingerprintSetV1;
use crate::link_object::{LayoutFingerprintV1, ScanFingerprintV1};

const LAYOUT_DOMAIN: &str = "scoop-layout-v1";
const SCAN_DOMAIN: &str = "scoop-scan-v1";
const STATIC_VALUE_LAYOUT_KIND: u32 = 1;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VerifiedStrongStaticStorageShapeFingerprintV1 {
    storage: PersistentStaticStorageId,
    layout: PersistentLayoutId,
    layout_node: DigestNodeId,
    layout_fingerprint: LayoutFingerprintV1,
    scan: PersistentScanId,
    scan_node: DigestNodeId,
    scan_fingerprint: ScanFingerprintV1,
}

impl VerifiedStrongStaticStorageShapeFingerprintV1 {
    pub const fn storage(self) -> PersistentStaticStorageId {
        self.storage
    }

    pub const fn layout(self) -> PersistentLayoutId {
        self.layout
    }

    pub const fn layout_node(self) -> DigestNodeId {
        self.layout_node
    }

    pub const fn layout_fingerprint(self) -> LayoutFingerprintV1 {
        self.layout_fingerprint
    }

    pub const fn scan(self) -> PersistentScanId {
        self.scan
    }

    pub const fn scan_node(self) -> DigestNodeId {
        self.scan_node
    }

    pub const fn scan_fingerprint(self) -> ScanFingerprintV1 {
        self.scan_fingerprint
    }
}

/// Layout and scan leaves rebuilt from the same closed static-storage plan
/// whose complete physical artifacts produced the storage definition leaves.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedStrongStaticStorageShapeFingerprintSetV1 {
    storage_definitions: VerifiedStrongStaticStorageDefinitionFingerprintSetV1,
    fingerprints: Vec<VerifiedStrongStaticStorageShapeFingerprintV1>,
}

impl VerifiedStrongStaticStorageShapeFingerprintSetV1 {
    pub const fn producer(&self) -> scoop_identity::ConeIdentity {
        self.storage_definitions.producer()
    }

    pub const fn storage_definitions(
        &self,
    ) -> &VerifiedStrongStaticStorageDefinitionFingerprintSetV1 {
        &self.storage_definitions
    }

    pub fn fingerprints(&self) -> &[VerifiedStrongStaticStorageShapeFingerprintV1] {
        &self.fingerprints
    }
}

pub fn compute_strong_static_storage_shape_fingerprints_v1(
    storage_definitions: VerifiedStrongStaticStorageDefinitionFingerprintSetV1,
) -> Result<
    VerifiedStrongStaticStorageShapeFingerprintSetV1,
    StrongStaticStorageShapeFingerprintError,
> {
    let registration_objects = storage_definitions.registration_objects();
    let registrations = registration_objects.registrations();
    let verified = registrations.registrations();
    let planned = registrations.plan().registrations();
    if verified.len() != planned.len()
        || verified.len() != registration_objects.fingerprints().len()
        || verified.len() != storage_definitions.fingerprints().len()
    {
        return Err(StrongStaticStorageShapeFingerprintError::ProofCoverageMismatch);
    }

    let mut fingerprints = Vec::with_capacity(verified.len());
    for (((verified, plan), registration_object), storage_definition) in verified
        .iter()
        .zip(planned)
        .zip(registration_objects.fingerprints())
        .zip(storage_definitions.fingerprints())
    {
        let semantic = plan.semantic();
        let storage = semantic.storage();
        if verified.storage() != storage
            || registration_object.storage() != storage
            || storage_definition.storage() != storage
            || registration_object.node() != plan.registration_object_node()
            || storage_definition.node() != plan.storage_definition_node()
        {
            return Err(StrongStaticStorageShapeFingerprintError::InputProofMismatch { storage });
        }
        let layout_fingerprint = static_layout_fingerprint(
            semantic.layout(),
            semantic.byte_size(),
            semantic.required_alignment(),
        )
        .map_err(
            |source| StrongStaticStorageShapeFingerprintError::LayoutHash { storage, source },
        )?;
        let scan_fingerprint = scan_fingerprint(semantic.scan_program()).map_err(|source| {
            StrongStaticStorageShapeFingerprintError::ScanHash { storage, source }
        })?;
        fingerprints.push(VerifiedStrongStaticStorageShapeFingerprintV1 {
            storage,
            layout: semantic.layout(),
            layout_node: plan.layout_fingerprint_node(),
            layout_fingerprint,
            scan: semantic.scan(),
            scan_node: plan.scan_fingerprint_node(),
            scan_fingerprint,
        });
    }

    Ok(VerifiedStrongStaticStorageShapeFingerprintSetV1 {
        storage_definitions,
        fingerprints,
    })
}

fn static_layout_fingerprint(
    layout: PersistentLayoutId,
    byte_size: u64,
    required_alignment: u64,
) -> Result<LayoutFingerprintV1, HashError> {
    domain_separated_runtime_hash(
        LAYOUT_DOMAIN,
        &StaticValueLayoutFingerprintInputV1 {
            layout,
            byte_size,
            required_alignment,
        },
    )
    .map(|digest| LayoutFingerprintV1(*digest.as_array()))
}

struct StaticValueLayoutFingerprintInputV1 {
    layout: PersistentLayoutId,
    byte_size: u64,
    required_alignment: u64,
}

impl RuntimeEncode for StaticValueLayoutFingerprintInputV1 {
    fn runtime_encode(&self, encoder: &mut RuntimeEncoder) -> Result<(), RuntimeEncodeError> {
        encoder.u32(STATIC_VALUE_LAYOUT_KIND)?;
        encoder.fixed(self.layout.as_array())?;
        encoder.u64(self.byte_size)?;
        encoder.u64(self.required_alignment)
    }
}

fn scan_fingerprint(scan: &RefScan) -> Result<ScanFingerprintV1, HashError> {
    domain_separated_runtime_hash(SCAN_DOMAIN, &StaticScanFingerprintInputV1(scan))
        .map(|digest| ScanFingerprintV1::from_array(*digest.as_array()))
}

struct StaticScanFingerprintInputV1<'a>(&'a RefScan);

impl RuntimeEncode for StaticScanFingerprintInputV1<'_> {
    fn runtime_encode(&self, encoder: &mut RuntimeEncoder) -> Result<(), RuntimeEncodeError> {
        match self.0 {
            RefScan::None => encoder.u32(0),
            RefScan::References(offsets) => {
                encoder.u32(1)?;
                encoder.sequence_length(offsets.len())?;
                for offset in offsets {
                    encoder.u64(*offset)?;
                }
                Ok(())
            }
            RefScan::Sequence(_) | RefScan::Array { .. } => {
                unreachable!("static registration plans reject non-value scans")
            }
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StrongStaticStorageShapeFingerprintError {
    ProofCoverageMismatch,
    InputProofMismatch {
        storage: PersistentStaticStorageId,
    },
    LayoutHash {
        storage: PersistentStaticStorageId,
        source: HashError,
    },
    ScanHash {
        storage: PersistentStaticStorageId,
        source: HashError,
    },
}

impl fmt::Display for StrongStaticStorageShapeFingerprintError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "failed to compute strong static-storage shape fingerprints: {self:?}"
        )
    }
}

impl std::error::Error for StrongStaticStorageShapeFingerprintError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::LayoutHash { source, .. } | Self::ScanHash { source, .. } => Some(source),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests;
