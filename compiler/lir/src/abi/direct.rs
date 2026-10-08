use super::*;

/// Complete physical plan for a register-passed logical value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AbiDirectValue {
    Scalar(AbiValue),
    DirectParts(AbiDirectParts),
}

impl From<AbiValue> for AbiDirectValue {
    fn from(value: AbiValue) -> Self {
        Self::Scalar(value)
    }
}

impl AbiDirectValue {
    pub const fn value(&self) -> &AbiValue {
        match self {
            Self::Scalar(value) => value,
            Self::DirectParts(parts) => parts.value(),
        }
    }

    pub const fn storage_type(&self) -> &LirType {
        self.value().storage_type()
    }

    pub const fn layout(&self) -> AbiNonZeroLayout {
        self.value().layout()
    }

    pub const fn scan(&self) -> &RefScan {
        self.value().scan()
    }

    pub const fn parameter_count(&self) -> usize {
        match self {
            Self::Scalar(_) => 1,
            Self::DirectParts(parts) => parts.parts().len(),
        }
    }
}

/// Interface values have two distinct pointer carriers. The object is the
/// only managed leaf; the itable belongs to immutable runtime metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AbiDirectParts {
    value: AbiValue,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AbiPart {
    pub byte_offset: u64,
    pub pointer_kind: crate::PointerKind,
}

impl AbiDirectParts {
    pub fn interface(value: AbiValue) -> Result<Self, AbiValueError> {
        if value.layout().size().get() != 16
            || value.layout().alignment().get() != 8
            || value.scan() != &RefScan::References(vec![0])
        {
            return Err(AbiValueError::InvalidInterfaceLayout);
        }
        Ok(Self { value })
    }

    pub const fn value(&self) -> &AbiValue {
        &self.value
    }

    pub const fn parts(&self) -> &'static [AbiPart; 2] {
        &[
            AbiPart {
                byte_offset: 0,
                pointer_kind: crate::PointerKind::Managed,
            },
            AbiPart {
                byte_offset: 8,
                pointer_kind: crate::PointerKind::Metadata,
            },
        ]
    }
}
