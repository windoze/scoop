//! Physical ABI carriers, with byte ranges independent of source field types.

use std::num::NonZeroU64;

use crate::FloatKind;

mod wire;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum PointerKind {
    Managed,
    Raw,
    Code,
    Metadata,
}

impl PointerKind {
    pub const fn dump(self) -> &'static str {
        match self {
            Self::Managed => "managed",
            Self::Raw => "raw",
            Self::Code => "code",
            Self::Metadata => "metadata",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum AbiArrayElement {
    I64,
    F32,
    F64,
}

/// Arrays retain the AArch64 consecutive-register constraint. FloatPair is
/// the SysV SSE <2 x float> carrier, not two independently allocated scalars.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum AbiCarrier {
    Integer(u8),
    Float(FloatKind),
    Pointer(PointerKind),
    FloatPair,
    Array { element: AbiArrayElement, count: u8 },
}

impl AbiCarrier {
    pub const fn byte_size(self) -> u64 {
        match self {
            Self::Integer(bits) => bits as u64 / 8,
            Self::Float(FloatKind::F32) => 4,
            Self::Float(FloatKind::F64) | Self::Pointer(_) | Self::FloatPair => 8,
            Self::Array { element, count } => {
                count as u64
                    * match element {
                        AbiArrayElement::F32 => 4,
                        AbiArrayElement::I64 | AbiArrayElement::F64 => 8,
                    }
            }
        }
    }

    fn is_valid(self) -> bool {
        match self {
            Self::Integer(bits) => bits > 0 && bits <= 128 && bits % 8 == 0,
            Self::Array {
                element: AbiArrayElement::I64,
                count,
            } => count == 2,
            Self::Array { count, .. } => (2..=4).contains(&count),
            Self::Float(_) | Self::Pointer(_) | Self::FloatPair => true,
        }
    }

    pub fn dump(self) -> String {
        match self {
            Self::Integer(bits) => format!("i{bits}"),
            Self::Float(FloatKind::F32) => "f32".into(),
            Self::Float(FloatKind::F64) => "f64".into(),
            Self::Pointer(kind) => format!("ptr<{}>", kind.dump()),
            Self::FloatPair => "<2xf32>".into(),
            Self::Array { element, count } => format!(
                "[{count}x{}]",
                match element {
                    AbiArrayElement::I64 => "i64",
                    AbiArrayElement::F32 => "f32",
                    AbiArrayElement::F64 => "f64",
                }
            ),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct AbiPart {
    carrier: AbiCarrier,
    byte_offset: u64,
    extent: NonZeroU64,
    alignment: NonZeroU64,
}

impl AbiPart {
    pub fn new(
        carrier: AbiCarrier,
        byte_offset: u64,
        extent: u64,
        alignment: u64,
    ) -> Result<Self, AbiCoercionError> {
        if !carrier.is_valid()
            || extent == 0
            || extent > carrier.byte_size()
            || !alignment.is_power_of_two()
            || byte_offset % alignment != 0
        {
            return Err(AbiCoercionError);
        }
        Ok(Self {
            carrier,
            byte_offset,
            extent: NonZeroU64::new(extent).ok_or(AbiCoercionError)?,
            alignment: NonZeroU64::new(alignment).ok_or(AbiCoercionError)?,
        })
    }

    pub const fn carrier(self) -> AbiCarrier {
        self.carrier
    }
    pub const fn byte_offset(self) -> u64 {
        self.byte_offset
    }
    pub const fn extent(self) -> u64 {
        self.extent.get()
    }
    pub const fn alignment(self) -> u64 {
        self.alignment.get()
    }
}

/// Supported targets use one grouped carrier or two eightbytes. This also
/// makes an empty direct-parts convention unrepresentable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum AbiCoercion {
    One(AbiPart),
    Two([AbiPart; 2]),
}

impl AbiCoercion {
    pub const fn parts(&self) -> &[AbiPart] {
        match self {
            Self::One(part) => std::slice::from_ref(part),
            Self::Two(parts) => parts,
        }
    }

    pub const fn interface() -> Self {
        let eight = NonZeroU64::new(8).unwrap();
        Self::Two([
            AbiPart {
                carrier: AbiCarrier::Pointer(PointerKind::Managed),
                byte_offset: 0,
                extent: eight,
                alignment: eight,
            },
            AbiPart {
                carrier: AbiCarrier::Pointer(PointerKind::Metadata),
                byte_offset: 8,
                extent: eight,
                alignment: eight,
            },
        ])
    }

    pub fn validate_storage(self, size: u64, alignment: u64) -> Result<(), AbiCoercionError> {
        let mut end = 0;
        for part in self.parts() {
            if part.byte_offset < end || part.alignment() > alignment {
                return Err(AbiCoercionError);
            }
            end = part
                .byte_offset
                .checked_add(part.extent())
                .ok_or(AbiCoercionError)?;
            if end > size {
                return Err(AbiCoercionError);
            }
        }
        Ok(())
    }

    pub fn has_managed_pointer(self) -> bool {
        self.parts()
            .iter()
            .any(|part| matches!(part.carrier, AbiCarrier::Pointer(PointerKind::Managed)))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AbiCoercionError;

impl std::fmt::Display for AbiCoercionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("ABI carrier or byte range is invalid")
    }
}
impl std::error::Error for AbiCoercionError {}
