use std::fmt;

use scoop_wire::{Encoder, HashError, WireEncodeV1};

use super::{CanonicalNativeLibraryName, CanonicalNativeNameError, SourceNativeSymbolV1};
use crate::ids::derive_persistent_id;
use crate::{NativeLinkRequirementId, PersistentNativeExternalSymbolId, TargetProfileWireId};

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct NativeLinkSymbolV1(Vec<u8>);

impl NativeLinkSymbolV1 {
    pub fn darwin_macho_external(
        logical: &SourceNativeSymbolV1,
    ) -> Result<Self, NativeLinkSymbolError> {
        let logical = logical.as_bytes();
        if logical.first() == Some(&0x01) {
            return Err(NativeLinkSymbolError::LlvmEscapePrefix);
        }
        let mut symbol = Vec::new();
        symbol
            .try_reserve_exact(logical.len() + 1)
            .map_err(|_| NativeLinkSymbolError::Allocation)?;
        symbol.push(b'_');
        symbol.extend_from_slice(logical);
        Ok(Self(symbol))
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }
}

impl WireEncodeV1 for NativeLinkSymbolV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.bytes(&self.0)
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct NativeExternalSymbolKeyV1 {
    target_profile: TargetProfileWireId,
    native_link_symbol: NativeLinkSymbolV1,
}

impl NativeExternalSymbolKeyV1 {
    pub fn darwin_macho_external(
        logical: &SourceNativeSymbolV1,
    ) -> Result<Self, NativeLinkSymbolError> {
        Ok(Self {
            target_profile: TargetProfileWireId::darwin_aarch64(),
            native_link_symbol: NativeLinkSymbolV1::darwin_macho_external(logical)?,
        })
    }

    pub fn target_profile(&self) -> &TargetProfileWireId {
        &self.target_profile
    }

    pub fn native_link_symbol(&self) -> &NativeLinkSymbolV1 {
        &self.native_link_symbol
    }
}

impl WireEncodeV1 for NativeExternalSymbolKeyV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        self.target_profile.encode(encoder)?;
        encoder.field(2)?;
        self.native_link_symbol.encode(encoder)
    }
}

impl PersistentNativeExternalSymbolId {
    pub fn from_key(key: &NativeExternalSymbolKeyV1) -> Result<Self, HashError> {
        derive_persistent_id("scoop-native-link-symbol-v1", key)
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CanonicalNativeGroupName(String);

impl CanonicalNativeGroupName {
    pub fn new(value: &str) -> Result<Self, CanonicalNativeNameError> {
        super::native_name::validate_native_name(value)?;
        Ok(Self(value.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl WireEncodeV1 for CanonicalNativeGroupName {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.text(&self.0)
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum NativeLibraryKindV1 {
    TargetDefault,
    Dynamic,
    StaticArchive,
    Framework,
}

impl WireEncodeV1 for NativeLibraryKindV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.unsigned(match self {
            Self::TargetDefault => 1,
            Self::Dynamic => 2,
            Self::StaticArchive => 3,
            Self::Framework => 4,
        })
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum NativeLibraryGroupingV1 {
    Independent,
    OrderedGroup {
        name: CanonicalNativeGroupName,
        position: u32,
    },
}

impl WireEncodeV1 for NativeLibraryGroupingV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::Independent => encode_empty_sum(encoder, 1),
            Self::OrderedGroup { name, position } => {
                encoder.map(3)?;
                encode_tag(encoder, 2)?;
                encoder.field(1)?;
                name.encode(encoder)?;
                encoder.field(2)?;
                encoder.unsigned(u64::from(*position))
            }
        }
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct NativeLinkRequirementKeyV1 {
    target_profile: TargetProfileWireId,
    library: CanonicalNativeLibraryName,
    kind: NativeLibraryKindV1,
    grouping: NativeLibraryGroupingV1,
}

impl NativeLinkRequirementKeyV1 {
    pub fn target_default(library: CanonicalNativeLibraryName) -> Self {
        Self {
            target_profile: TargetProfileWireId::darwin_aarch64(),
            library,
            kind: NativeLibraryKindV1::TargetDefault,
            grouping: NativeLibraryGroupingV1::Independent,
        }
    }

    pub fn for_darwin(
        library: CanonicalNativeLibraryName,
        kind: NativeLibraryKindV1,
        grouping: NativeLibraryGroupingV1,
    ) -> Self {
        Self {
            target_profile: TargetProfileWireId::darwin_aarch64(),
            library,
            kind,
            grouping,
        }
    }

    pub fn target_profile(&self) -> &TargetProfileWireId {
        &self.target_profile
    }
}

impl WireEncodeV1 for NativeLinkRequirementKeyV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(4)?;
        encoder.field(1)?;
        self.target_profile.encode(encoder)?;
        encoder.field(2)?;
        self.library.encode(encoder)?;
        encoder.field(3)?;
        self.kind.encode(encoder)?;
        encoder.field(4)?;
        self.grouping.encode(encoder)
    }
}

impl NativeLinkRequirementId {
    pub fn from_key(key: &NativeLinkRequirementKeyV1) -> Result<Self, HashError> {
        derive_persistent_id("scoop-native-link-requirement-v1", key)
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum NativeLibraryBindingV1 {
    DefaultNativeNamespace,
    Requirement(NativeLinkRequirementId),
}

impl WireEncodeV1 for NativeLibraryBindingV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::DefaultNativeNamespace => encode_empty_sum(encoder, 1),
            Self::Requirement(id) => encode_value_sum(encoder, 2, id),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeLinkSymbolError {
    LlvmEscapePrefix,
    Allocation,
}

impl fmt::Display for NativeLinkSymbolError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::LlvmEscapePrefix => {
                "logical native symbol must not start with the LLVM escape byte"
            }
            Self::Allocation => "failed to allocate normalized native link symbol",
        })
    }
}

impl std::error::Error for NativeLinkSymbolError {}

fn encode_tag(encoder: &mut Encoder, tag: u64) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.field(0)?;
    encoder.unsigned(tag)
}

fn encode_empty_sum(encoder: &mut Encoder, tag: u64) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.map(1)?;
    encode_tag(encoder, tag)
}

fn encode_value_sum(
    encoder: &mut Encoder,
    tag: u64,
    value: &impl WireEncodeV1,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.map(2)?;
    encode_tag(encoder, tag)?;
    encoder.field(1)?;
    value.encode(encoder)
}

#[cfg(test)]
mod tests {
    use scoop_wire::encode;

    use super::{NativeExternalSymbolKeyV1, NativeLinkRequirementKeyV1, NativeLinkSymbolV1};
    use crate::{
        CanonicalNativeLibraryName, NativeLinkRequirementId, PersistentNativeExternalSymbolId,
        SourceNativeSymbolV1,
    };

    #[test]
    fn macho_normalization_preserves_existing_underscore() {
        let plain = SourceNativeSymbolV1::new("foo").unwrap();
        let underscored = SourceNativeSymbolV1::new("_foo").unwrap();
        assert_eq!(
            NativeLinkSymbolV1::darwin_macho_external(&plain)
                .unwrap()
                .as_bytes(),
            b"_foo"
        );
        assert_eq!(
            NativeLinkSymbolV1::darwin_macho_external(&underscored)
                .unwrap()
                .as_bytes(),
            b"__foo"
        );
    }

    #[test]
    fn native_symbol_identity_has_fixed_vector() {
        let logical = SourceNativeSymbolV1::new("foo").unwrap();
        let key = NativeExternalSymbolKeyV1::darwin_macho_external(&logical).unwrap();
        assert_eq!(
            hex(&encode(&key).unwrap()),
            "a201a301781d6f72672e73636f6f702d6c616e672e7461726765742d70726f66696c65026e64617277696e2d61617263683634030102445f666f6f"
        );
        assert_eq!(
            PersistentNativeExternalSymbolId::from_key(&key)
                .unwrap()
                .to_string(),
            "beea7e932b9d779c847d70820abc92cc814022cb653fcdb07519f152f51e995f"
        );
    }

    #[test]
    fn target_default_requirement_has_fixed_vector() {
        let key = NativeLinkRequirementKeyV1::target_default(
            CanonicalNativeLibraryName::new("sample").unwrap(),
        );
        assert_eq!(
            hex(&encode(&key).unwrap()),
            "a401a301781d6f72672e73636f6f702d6c616e672e7461726765742d70726f66696c65026e64617277696e2d616172636836340301026673616d706c65030104a10001"
        );
        assert_eq!(
            NativeLinkRequirementId::from_key(&key).unwrap().to_string(),
            "d89eb1c8a7b896c0af0b968bd772bbf71444df18faabb0b29c9670f12fbc7ae0"
        );
    }

    fn hex(bytes: &[u8]) -> String {
        bytes.iter().map(|byte| format!("{byte:02x}")).collect()
    }
}
