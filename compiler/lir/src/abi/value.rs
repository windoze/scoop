use super::*;

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
