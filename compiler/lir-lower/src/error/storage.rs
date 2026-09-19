use scoop_lir as lir;

/// Fallible physical construction inside MIR-to-LIR lowering. No invalid
/// geometry is converted to a placeholder shape or deferred backend panic.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StorageLoweringError {
    Replay(lir::StorageReplayError),
    Shape(lir::TypeInstanceShapeError),
    Scan(lir::RefScanValidationError),
    AbiLayout(lir::AbiLayoutError),
    AbiValue(lir::AbiValueError),
    BoxDescriptor(lir::BoxDescriptorError),
    AbiClassification(lir::ScoopAbiClassificationError),
    InvalidRepresentation(&'static str),
}

pub(crate) type StorageResult<T> = Result<T, StorageLoweringError>;

macro_rules! from_error {
    ($type:ty, $variant:ident) => {
        impl From<$type> for StorageLoweringError {
            fn from(source: $type) -> Self {
                Self::$variant(source)
            }
        }
    };
}

from_error!(lir::StorageReplayError, Replay);
from_error!(lir::TypeInstanceShapeError, Shape);
from_error!(lir::RefScanValidationError, Scan);
from_error!(lir::AbiLayoutError, AbiLayout);
from_error!(lir::AbiValueError, AbiValue);
from_error!(lir::BoxDescriptorError, BoxDescriptor);
from_error!(lir::ScoopAbiClassificationError, AbiClassification);

impl std::fmt::Display for StorageLoweringError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "LIR storage layout failed: {self:?}")
    }
}
impl std::error::Error for StorageLoweringError {}

impl From<StorageLoweringError> for super::StrongLirLoweringError {
    fn from(source: StorageLoweringError) -> Self {
        Self::StorageReplay(source)
    }
}
