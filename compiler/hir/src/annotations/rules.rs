use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError, WireErrorKind};

#[cfg(test)]
mod tests;

/// Categories describe actual source targets, independently of use-site syntax.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum AnnotationTargetKind {
    Class = 1,
    AnnotationClass,
    EnumVariant,
    Field,
    Property,
    Function,
    Constructor,
    ValueParameter,
    PropertyGetter,
    PropertySetter,
    TypeAlias,
    Type,
}

impl AnnotationTargetKind {
    pub const ALL: [Self; 12] = [
        Self::Class,
        Self::AnnotationClass,
        Self::EnumVariant,
        Self::Field,
        Self::Property,
        Self::Function,
        Self::Constructor,
        Self::ValueParameter,
        Self::PropertyGetter,
        Self::PropertySetter,
        Self::TypeAlias,
        Self::Type,
    ];

    pub fn from_value(value: u64) -> Option<Self> {
        Self::ALL
            .get(usize::try_from(value.checked_sub(1)?).ok()?)
            .copied()
    }

    pub const fn name(self) -> &'static str {
        match self {
            Self::Class => "Class",
            Self::AnnotationClass => "AnnotationClass",
            Self::EnumVariant => "EnumVariant",
            Self::Field => "Field",
            Self::Property => "Property",
            Self::Function => "Function",
            Self::Constructor => "Constructor",
            Self::ValueParameter => "ValueParameter",
            Self::PropertyGetter => "PropertyGetter",
            Self::PropertySetter => "PropertySetter",
            Self::TypeAlias => "TypeAlias",
            Self::Type => "Type",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AnnotationRules {
    allowed: u16,
    pub repeatable: bool,
}

impl Default for AnnotationRules {
    fn default() -> Self {
        Self {
            allowed: Self::ALL_TARGETS,
            repeatable: false,
        }
    }
}

impl AnnotationRules {
    const ALL_TARGETS: u16 = (1 << AnnotationTargetKind::ALL.len()) - 1;

    pub const fn none() -> Self {
        Self {
            allowed: 0,
            repeatable: false,
        }
    }

    pub const fn only(kind: AnnotationTargetKind) -> Self {
        Self {
            allowed: 1 << (kind as u8 - 1),
            repeatable: false,
        }
    }

    pub const fn permits(self, kind: AnnotationTargetKind) -> bool {
        self.allowed & (1 << (kind as u8 - 1)) != 0
    }

    pub fn insert(&mut self, kind: AnnotationTargetKind) -> bool {
        let fresh = !self.permits(kind);
        self.allowed |= 1 << (kind as u8 - 1);
        fresh
    }
}

impl WireEncode for AnnotationRules {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        encoder.unsigned(u64::from(self.allowed))?;
        encoder.field(2)?;
        encoder.unsigned(u64::from(self.repeatable))
    }
}

impl WireDecode for AnnotationRules {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(2)?;
        let allowed = decoder.field(1, Decoder::unsigned)?;
        let repeatable = decoder.field(2, Decoder::unsigned)?;
        if allowed > u64::from(Self::ALL_TARGETS) || repeatable > 1 {
            return Err(WireError::new(
                WireErrorKind::IntegerOutOfRange,
                decoder.path().clone(),
                Some(decoder.position()),
            ));
        }
        Ok(Self {
            allowed: allowed as u16,
            repeatable: repeatable != 0,
        })
    }
}
