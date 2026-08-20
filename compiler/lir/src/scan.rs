//! Checked runtime scan constituents. These prove scan semantics, not the
//! ownership or availability of a cross-Cone layout definition.

use scoop_wire::{CanonicalHashStream, Digest256};

use crate::RefScan;

mod normal;
mod ranges;
mod wire;

pub use wire::DecodedRefScanV1;
pub(crate) use wire::encode_scan as encode_canonical_scan;

/// The runtime `scoop-scan-v1` fingerprint, independent of Wire CBOR.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct CanonicalScanFingerprintV1(Digest256);

impl CanonicalScanFingerprintV1 {
    pub const fn as_array(&self) -> &[u8; 32] {
        self.0.as_array()
    }
}

/// A scan in runtime v1 normal form.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedRefScanV1 {
    scan: RefScan,
    bytes: Vec<u8>,
    fingerprint: CanonicalScanFingerprintV1,
}

impl CheckedRefScanV1 {
    /// Producer entry: flatten, merge, sort and deduplicate to a fixed point.
    pub fn normalize(scan: RefScan) -> Result<Self, RefScanValidationError> {
        let scan = normal::normalize(scan)?;
        Self::from_canonical(scan)
    }

    /// Reader entry: reject noncanonical input without repairing it.
    pub fn from_canonical(scan: RefScan) -> Result<Self, RefScanValidationError> {
        let bytes = normal::canonical_bytes(&scan)?;
        let fingerprint = fingerprint(&bytes);
        Ok(Self {
            scan,
            bytes,
            fingerprint,
        })
    }

    pub const fn as_ref_scan(&self) -> &RefScan {
        &self.scan
    }

    pub fn into_ref_scan(self) -> RefScan {
        self.scan
    }

    pub fn canonical_bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub const fn fingerprint(&self) -> CanonicalScanFingerprintV1 {
        self.fingerprint
    }

    /// Checks static ranges relative to a storage base. Dynamic Array counts
    /// still require runtime bounds checks against this extent.
    pub fn validate_extent(
        &self,
        extent: u64,
        alignment: u64,
    ) -> Result<(), RefScanValidationError> {
        ranges::validate(&self.scan, extent, alignment)
    }

    /// Move object-relative offsets only; an Array child remains relative to
    /// its element. Translation can change fingerprint ordering in Sequence.
    pub fn translated(&self, delta: u64) -> Result<Self, RefScanValidationError> {
        Self::normalize(normal::translate(&self.scan, delta)?)
    }
}

fn fingerprint(bytes: &[u8]) -> CanonicalScanFingerprintV1 {
    let mut stream = CanonicalHashStream::new();
    stream
        .update_byte_span(b"scoop-scan-v1")
        .expect("fixed hash domain fits u64");
    stream.update_raw(bytes);
    CanonicalScanFingerprintV1(stream.finalize())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RefScanValidationError {
    Cycle,
    EmptyReferences,
    UnorderedReferences,
    NonCanonicalSequence,
    EmptyArrayElement,
    ZeroArrayStride,
    OffsetOverflow,
    MisalignedReference { offset: u64 },
    MisalignedStorage { alignment: u64 },
    OutOfBounds { offset: u64, size: u64, extent: u64 },
    OverlappingRanges,
    ArrayPrefixOverlap,
    MisalignedArrayStride { stride: u64 },
}

impl std::fmt::Display for RefScanValidationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "invalid runtime reference scan: {self:?}")
    }
}

impl std::error::Error for RefScanValidationError {}

#[cfg(test)]
mod tests;
