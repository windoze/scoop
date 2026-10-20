use std::fmt;

use scoop_identity::{
    DigestNodeId, PersistentLayoutId, PersistentScanId, PersistentStaticStorageId,
};
use scoop_lir::RefScan;
use scoop_wire::{
    HashError, RuntimeEncode, RuntimeEncodeError, RuntimeEncoder, domain_separated_runtime_hash,
};

use super::StaticStorageRegistrationDataV1;
use crate::link_object::{LayoutFingerprintV1, ScanFingerprintV1};

const LAYOUT_DOMAIN: &str = "scoop-layout-v1";
const SCAN_DOMAIN: &str = "scoop-scan-v1";
const STATIC_VALUE_LAYOUT_KIND: u32 = 1;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StaticStorageShapeFingerprintV1 {
    storage: PersistentStaticStorageId,
    layout: PersistentLayoutId,
    layout_node: DigestNodeId,
    layout_fingerprint: LayoutFingerprintV1,
    scan: PersistentScanId,
    scan_node: DigestNodeId,
    scan_fingerprint: ScanFingerprintV1,
}

impl StaticStorageShapeFingerprintV1 {
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

/// Layout and scan fingerprints computed from the generated static-storage plan.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StaticStorageShapeFingerprintSetV1 {
    registrations: StaticStorageRegistrationDataV1,
    fingerprints: Vec<StaticStorageShapeFingerprintV1>,
}

impl StaticStorageShapeFingerprintSetV1 {
    pub const fn producer(&self) -> scoop_identity::ConeIdentity {
        self.registrations.producer()
    }

    pub const fn registrations(&self) -> &StaticStorageRegistrationDataV1 {
        &self.registrations
    }

    pub fn fingerprints(&self) -> &[StaticStorageShapeFingerprintV1] {
        &self.fingerprints
    }
}

pub fn compute_strong_static_storage_shape_fingerprints_v1(
    registrations: StaticStorageRegistrationDataV1,
) -> Result<StaticStorageShapeFingerprintSetV1, StrongStaticStorageShapeFingerprintError> {
    let planned = registrations.registrations();
    let mut fingerprints = Vec::with_capacity(planned.len());
    for plan in planned {
        let semantic = plan.semantic();
        let storage = semantic.storage();
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
        fingerprints.push(StaticStorageShapeFingerprintV1 {
            storage,
            layout: semantic.layout(),
            layout_node: plan.layout_fingerprint_node(),
            layout_fingerprint,
            scan: semantic.scan(),
            scan_node: plan.scan_fingerprint_node(),
            scan_fingerprint,
        });
    }

    Ok(StaticStorageShapeFingerprintSetV1 {
        registrations,
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
        }
    }
}

#[cfg(test)]
mod tests;
