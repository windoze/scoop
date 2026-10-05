use std::fmt;

use scoop_wire::{Encoder, HashError, WireEncode, domain_separated_cbor_hash};

use super::{GENERATED_C_SOURCE_TEMPLATE_DOMAIN, encode_unsigned_field, write_hex};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct GeneratedCSourceTemplateContractV1;

impl GeneratedCSourceTemplateContractV1 {
    pub const CURRENT: Self = Self;

    pub const fn components(self) -> &'static [GeneratedCSourceTemplateComponentV1] {
        &GeneratedCSourceTemplateComponentV1::ALL
    }

    pub fn fingerprint(self) -> Result<GeneratedCSourceTemplateFingerprint, HashError> {
        domain_separated_cbor_hash(GENERATED_C_SOURCE_TEMPLATE_DOMAIN, &self)
            .map(|digest| GeneratedCSourceTemplateFingerprint(*digest.as_array()))
    }
}

impl WireEncode for GeneratedCSourceTemplateContractV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.array(GeneratedCSourceTemplateComponentV1::ALL.len() as u64)?;
        for component in GeneratedCSourceTemplateComponentV1::ALL {
            encoder.map(2)?;
            encoder.field(1)?;
            component.encode(encoder)?;
            encode_unsigned_field(encoder, 2, 1)?;
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum GeneratedCSourceTemplateComponentV1 {
    TypeRenderer,
    LayoutAssertions,
    ExternalDeclarations,
    OutboundWrappers,
    NativeGlobalAccessors,
    CallbackTrampolines,
    ForeignCallbackTrampolines,
    UnitObjectPartition,
    AtomBoundaryMaterialization,
}

impl GeneratedCSourceTemplateComponentV1 {
    pub const ALL: [Self; 9] = [
        Self::TypeRenderer,
        Self::LayoutAssertions,
        Self::ExternalDeclarations,
        Self::OutboundWrappers,
        Self::NativeGlobalAccessors,
        Self::CallbackTrampolines,
        Self::ForeignCallbackTrampolines,
        Self::UnitObjectPartition,
        Self::AtomBoundaryMaterialization,
    ];

    const fn tag(self) -> u32 {
        match self {
            Self::TypeRenderer => 1,
            Self::LayoutAssertions => 2,
            Self::ExternalDeclarations => 3,
            Self::OutboundWrappers => 4,
            Self::NativeGlobalAccessors => 5,
            Self::CallbackTrampolines => 6,
            Self::ForeignCallbackTrampolines => 7,
            Self::UnitObjectPartition => 8,
            Self::AtomBoundaryMaterialization => 9,
        }
    }
}

impl WireEncode for GeneratedCSourceTemplateComponentV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.unsigned(u64::from(self.tag()))
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct GeneratedCSourceTemplateFingerprint([u8; 32]);

impl GeneratedCSourceTemplateFingerprint {
    pub const fn as_array(&self) -> &[u8; 32] {
        &self.0
    }
}

impl WireEncode for GeneratedCSourceTemplateFingerprint {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.bytes(&self.0)
    }
}

impl fmt::Display for GeneratedCSourceTemplateFingerprint {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write_hex(formatter, &self.0)
    }
}
