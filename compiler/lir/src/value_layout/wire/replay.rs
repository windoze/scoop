use scoop_wire::{BudgetMeter, WireError, WirePath};

use super::*;
use crate::MeteredScanValidationError;

enum ExpectedStorage<'a> {
    Zero(NonZeroPow2),
    Inline {
        extent: u64,
        alignment: NonZeroPow2,
        scan: &'a CheckedRefScanV1,
    },
}

impl DecodedValueStorageLayoutV1 {
    pub fn validate_against(
        self,
        expected: &ValueStorageLayoutV1,
        meter: &mut BudgetMeter,
    ) -> Result<(), StorageWireReplayError> {
        self.0.validate_against(
            match expected.kind() {
                ValueStorageKindV1::ZeroSized { alignment } => ExpectedStorage::Zero(alignment),
                ValueStorageKindV1::Inline {
                    size,
                    alignment,
                    scan,
                } => ExpectedStorage::Inline {
                    extent: size.get(),
                    alignment,
                    scan,
                },
            },
            meter,
        )
    }
}

impl DecodedArrayElementStorageV1 {
    pub fn validate_against(
        self,
        expected: &ArrayElementStorageV1,
        meter: &mut BudgetMeter,
    ) -> Result<(), StorageWireReplayError> {
        self.0.validate_against(
            match expected.kind() {
                ArrayElementStorageKindV1::ZeroSized { alignment } => {
                    ExpectedStorage::Zero(alignment)
                }
                ArrayElementStorageKindV1::Inline {
                    stride,
                    alignment,
                    scan,
                } => ExpectedStorage::Inline {
                    extent: stride.get(),
                    alignment,
                    scan,
                },
            },
            meter,
        )
    }
}

impl RawStorage {
    fn validate_against(
        self,
        expected: ExpectedStorage<'_>,
        meter: &mut BudgetMeter,
    ) -> Result<(), StorageWireReplayError> {
        meter.charge_work(3, &WirePath::root())?;
        match (self, expected) {
            (Self::ZeroSized { alignment }, ExpectedStorage::Zero(expected))
                if alignment == expected.get() =>
            {
                Ok(())
            }
            (
                Self::NonZero {
                    extent,
                    alignment,
                    scan,
                },
                ExpectedStorage::Inline {
                    extent: expected_extent,
                    alignment: expected_alignment,
                    scan: expected_scan,
                },
            ) if extent == expected_extent && alignment == expected_alignment.get() => {
                let scan = scan.validate_metered(meter)?;
                if &scan == expected_scan {
                    Ok(())
                } else {
                    Err(StorageWireReplayError::StorageMismatch)
                }
            }
            _ => Err(StorageWireReplayError::StorageMismatch),
        }
    }
}

#[derive(Debug)]
pub enum StorageWireReplayError {
    StorageMismatch,
    Scan(MeteredScanValidationError),
    Resource(WireError),
}
impl From<MeteredScanValidationError> for StorageWireReplayError {
    fn from(error: MeteredScanValidationError) -> Self {
        Self::Scan(error)
    }
}
impl From<WireError> for StorageWireReplayError {
    fn from(error: WireError) -> Self {
        Self::Resource(error)
    }
}
impl std::fmt::Display for StorageWireReplayError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "storage wire differs from replay: {self:?}")
    }
}
impl std::error::Error for StorageWireReplayError {}
