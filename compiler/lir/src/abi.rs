use std::num::NonZeroU64;

use super::{
    CallingConvention, EnumDefId, EnumDefs, EnumRepr, LirTargetProfile, LirType, LocalId, RefScan,
    ScoopAbiPassing, ScoopAbiValueShape, Value,
};

static EMPTY_REF_SCAN: RefScan = RefScan::None;

mod direct;
pub use direct::{AbiDirectParts, AbiDirectValue, AbiPart};

/// Failure to construct one of the refined Scoop ABI storage layouts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AbiLayoutError {
    ZeroSize,
    ZeroAlignment,
    AlignmentNotPowerOfTwo(u64),
}

/// Failure to bind an exact LIR storage type to an ABI value or address.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AbiValueError {
    VoidStorageType,
    InvalidInterfaceLayout,
    StorageTypeMismatch { expected: LirType, actual: LirType },
}

/// Failure to derive the physical Scoop ABI shape of a stored LIR value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScoopAbiClassificationError {
    VoidStorageType,
    InvalidEnumDefinition(EnumDefId),
}

/// Classifies one non-zero logical LIR value with the module's target profile.
///
/// Layout lowering handles zero-sized values before calling this function.
/// Keeping the exact type-to-shape mapping here gives MIR -> LIR lowering and
/// codegen boundary validation one authority while the target profile owns the
/// final shape-to-passing decision.
pub fn classify_non_zero_scoop_abi_value(
    profile: LirTargetProfile,
    enums: &EnumDefs,
    ty: &LirType,
) -> Result<ScoopAbiPassing, ScoopAbiClassificationError> {
    Ok(profile.classify_scoop_abi_value(scoop_abi_value_shape(enums, ty)?))
}

/// Return the target-independent scalar/aggregate shape consumed by the
/// closed Scoop ABI classifier. Zero-sized values use the same shape even
/// though their physical parameter is elided.
pub fn scoop_abi_value_shape(
    enums: &EnumDefs,
    ty: &LirType,
) -> Result<ScoopAbiValueShape, ScoopAbiClassificationError> {
    let shape = match ty {
        LirType::I1
        | LirType::I8
        | LirType::I16
        | LirType::I32
        | LirType::F32
        | LirType::F64
        | LirType::I64
        | LirType::MachineScalar(_)
        | LirType::Ptr(_) => ScoopAbiValueShape::Scalar,
        LirType::Interface => ScoopAbiValueShape::Interface,
        LirType::Enum(id) => {
            let definition = enums
                .get(*id)
                .ok_or(ScoopAbiClassificationError::InvalidEnumDefinition(*id))?;
            match definition.repr {
                EnumRepr::Niche {
                    kind: crate::NullNicheKind::Interface,
                    ..
                } => ScoopAbiValueShape::Interface,
                EnumRepr::Niche { .. } => ScoopAbiValueShape::Scalar,
                EnumRepr::Tagged { .. } => ScoopAbiValueShape::Aggregate,
            }
        }
        LirType::Aggregate(_) | LirType::Struct(_) | LirType::ExceptionRecord => {
            ScoopAbiValueShape::Aggregate
        }
        LirType::Void => return Err(ScoopAbiClassificationError::VoidStorageType),
    };
    Ok(shape)
}

fn checked_alignment(alignment: u64) -> Result<NonZeroU64, AbiLayoutError> {
    let alignment = NonZeroU64::new(alignment).ok_or(AbiLayoutError::ZeroAlignment)?;
    if alignment.get().is_power_of_two() {
        Ok(alignment)
    } else {
        Err(AbiLayoutError::AlignmentNotPowerOfTwo(alignment.get()))
    }
}

/// Checked layout of an exact zero-sized Scoop value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AbiZeroSizedLayout {
    alignment: NonZeroU64,
}

impl AbiZeroSizedLayout {
    pub fn new(alignment: u64) -> Result<Self, AbiLayoutError> {
        Ok(Self {
            alignment: checked_alignment(alignment)?,
        })
    }

    pub const fn size(self) -> u64 {
        0
    }

    pub const fn alignment(self) -> NonZeroU64 {
        self.alignment
    }
}

/// Checked layout of an exact non-zero-sized Scoop value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AbiNonZeroLayout {
    size: NonZeroU64,
    alignment: NonZeroU64,
}

impl AbiNonZeroLayout {
    pub fn new(size: u64, alignment: u64) -> Result<Self, AbiLayoutError> {
        Ok(Self {
            size: NonZeroU64::new(size).ok_or(AbiLayoutError::ZeroSize)?,
            alignment: checked_alignment(alignment)?,
        })
    }

    pub const fn size(self) -> NonZeroU64 {
        self.size
    }

    pub const fn alignment(self) -> NonZeroU64 {
        self.alignment
    }
}

/// A logical Scoop value that occupies no physical ABI storage.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AbiZst {
    storage_type: LirType,
    layout: AbiZeroSizedLayout,
}

impl AbiZst {
    pub fn new(storage_type: LirType, layout: AbiZeroSizedLayout) -> Result<Self, AbiValueError> {
        if storage_type == LirType::Void {
            return Err(AbiValueError::VoidStorageType);
        }
        Ok(Self {
            storage_type,
            layout,
        })
    }

    pub const fn storage_type(&self) -> &LirType {
        &self.storage_type
    }

    pub const fn layout(&self) -> AbiZeroSizedLayout {
        self.layout
    }

    pub fn scan(&self) -> &'static RefScan {
        &EMPTY_REF_SCAN
    }
}

/// An exact, non-zero-sized Scoop value together with its complete root scan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AbiValue {
    storage_type: LirType,
    layout: AbiNonZeroLayout,
    scan: RefScan,
}

impl AbiValue {
    pub fn new(
        storage_type: LirType,
        layout: AbiNonZeroLayout,
        scan: RefScan,
    ) -> Result<Self, AbiValueError> {
        if storage_type == LirType::Void {
            return Err(AbiValueError::VoidStorageType);
        }
        Ok(Self {
            storage_type,
            layout,
            scan,
        })
    }

    pub const fn storage_type(&self) -> &LirType {
        &self.storage_type
    }

    pub const fn layout(&self) -> AbiNonZeroLayout {
        self.layout
    }

    pub const fn scan(&self) -> &RefScan {
        &self.scan
    }
}

/// Exact typed local storage used to pass one indirect Scoop ABI argument.
///
/// Construction checks the local's declared storage type against the ABI
/// value whose address the call signature requires. The private local id
/// prevents later stages from pairing an unrelated slot with that signature.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AbiArgumentStorage(LocalId);

impl AbiArgumentStorage {
    pub fn new(
        local: LocalId,
        local_storage_type: &LirType,
        expected: &AbiValue,
    ) -> Result<Self, AbiValueError> {
        if local_storage_type == expected.storage_type() {
            Ok(Self(local))
        } else {
            Err(AbiValueError::StorageTypeMismatch {
                expected: expected.storage_type().clone(),
                actual: local_storage_type.clone(),
            })
        }
    }

    pub const fn local(self) -> LocalId {
        self.0
    }
}

/// One logical call argument together with its selected physical convention.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AbiCallArgument {
    ElidedZst(Value),
    Direct(Value),
    Indirect(AbiArgumentStorage),
}

impl AbiCallArgument {
    pub const fn logical_value(self) -> Value {
        match self {
            Self::ElidedZst(value) | Self::Direct(value) => value,
            Self::Indirect(storage) => Value::Local(storage.local()),
        }
    }
}

/// Physical passing convention selected for one logical Scoop argument.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AbiArgument {
    ElidedZst(AbiZst),
    Direct(AbiDirectValue),
    Indirect(AbiValue),
}

impl AbiArgument {
    pub const fn logical_storage_type(&self) -> &LirType {
        match self {
            Self::ElidedZst(value) => value.storage_type(),
            Self::Direct(value) => value.storage_type(),
            Self::Indirect(value) => value.storage_type(),
        }
    }

    pub fn scan(&self) -> &RefScan {
        match self {
            Self::ElidedZst(value) => value.scan(),
            Self::Direct(value) => value.scan(),
            Self::Indirect(value) => value.scan(),
        }
    }

    pub const fn physical_value(&self) -> Option<&AbiValue> {
        match self {
            Self::ElidedZst(_) => None,
            Self::Direct(value) => Some(value.value()),
            Self::Indirect(value) => Some(value),
        }
    }

    pub const fn parameter_count(&self) -> usize {
        match self {
            Self::ElidedZst(_) => 0,
            Self::Direct(value) => value.parameter_count(),
            Self::Indirect(_) => 1,
        }
    }

    pub const fn is_elided(&self) -> bool {
        matches!(self, Self::ElidedZst(_))
    }

    pub const fn is_indirect(&self) -> bool {
        matches!(self, Self::Indirect(_))
    }
}

/// Physical return convention selected for one logical Scoop result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AbiReturn {
    UnitVoid,
    ElidedZst(AbiZst),
    Direct(AbiDirectValue),
    Indirect(AbiValue),
}

impl AbiReturn {
    pub const fn logical_storage_type(&self) -> Option<&LirType> {
        match self {
            Self::UnitVoid => None,
            Self::ElidedZst(value) => Some(value.storage_type()),
            Self::Direct(value) => Some(value.storage_type()),
            Self::Indirect(value) => Some(value.storage_type()),
        }
    }

    pub fn scan(&self) -> Option<&RefScan> {
        match self {
            Self::UnitVoid => None,
            Self::ElidedZst(value) => Some(value.scan()),
            Self::Direct(value) => Some(value.scan()),
            Self::Indirect(value) => Some(value.scan()),
        }
    }

    pub const fn physical_value(&self) -> Option<&AbiValue> {
        match self {
            Self::Direct(value) => Some(value.value()),
            Self::Indirect(value) => Some(value),
            Self::UnitVoid | Self::ElidedZst(_) => None,
        }
    }

    pub const fn is_indirect(&self) -> bool {
        matches!(self, Self::Indirect(_))
    }
}

mod signature;
pub use signature::{
    AbiArgumentLocation, AbiPhysicalParameter, AbiPhysicalParameterOrigin, ScoopAbiSignature,
};

#[cfg(test)]
mod tests;
