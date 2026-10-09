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

/// A complete carrier plan for one logical value. GC-free aggregates use
/// target coercions; interfaces retain their distinct object and metadata parts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AbiDirectParts {
    value: AbiValue,
    coercion: crate::AbiCoercion,
}

impl AbiDirectParts {
    pub fn interface(value: AbiValue) -> Result<Self, AbiValueError> {
        if value.layout().size().get() != 16
            || value.layout().alignment().get() != 8
            || value.scan() != &RefScan::References(vec![0])
        {
            return Err(AbiValueError::InvalidInterfaceLayout);
        }
        Ok(Self {
            value,
            coercion: crate::AbiCoercion::interface(),
        })
    }

    pub const fn value(&self) -> &AbiValue {
        &self.value
    }

    pub fn new(value: AbiValue, coercion: crate::AbiCoercion) -> Result<Self, AbiValueError> {
        coercion
            .validate_storage(
                value.layout().size().get(),
                value.layout().alignment().get(),
            )
            .map_err(|_| AbiValueError::InvalidCoercion)?;
        if coercion.has_managed_pointer() {
            let interface = Self::interface(value)?;
            if coercion != interface.coercion {
                return Err(AbiValueError::InvalidCoercion);
            }
            return Ok(interface);
        }
        if value.scan() != &RefScan::None {
            return Err(AbiValueError::InvalidCoercion);
        }
        Ok(Self { value, coercion })
    }

    pub const fn coercion(&self) -> crate::AbiCoercion {
        self.coercion
    }
    pub const fn parts(&self) -> &[crate::AbiPart] {
        self.coercion.parts()
    }
}
