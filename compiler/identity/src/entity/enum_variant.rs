use std::fmt;

use scoop_wire::{Encoder, HashError, WireEncode};

use super::{
    GeneratedNominalIdentityError, GeneratedNominalKey, NominalDeclarationOwner,
    SourceDeclarationIdentityError, SourceDeclarationKey, SourceDeclarationKind,
};
use crate::ids::derive_persistent_id;
use crate::{
    CanonicalIdentifier, PersistentEnumVariantFieldId, PersistentEnumVariantId, PersistentTypeId,
};

mod decode;

pub use decode::{
    DecodedEnumVariantFieldKey, DecodedEnumVariantFieldSelector, DecodedEnumVariantIdentityKey,
    EnumVariantFieldResolutionError, EnumVariantResolutionError,
};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum GeneratedEnumVariantRole {
    CoroutineStepCompleted,
    CoroutineStepSuspended,
    CoroutineSlotEmpty,
    CoroutineSlotValue,
}

impl WireEncode for GeneratedEnumVariantRole {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.unsigned(match self {
            Self::CoroutineStepCompleted => 1,
            Self::CoroutineStepSuspended => 2,
            Self::CoroutineSlotEmpty => 3,
            Self::CoroutineSlotValue => 4,
        })
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct EnumVariantIdentityKey(EnumVariantIdentityKeyKind);

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
enum EnumVariantIdentityKeyKind {
    Source {
        owner: NominalDeclarationOwner,
        name: CanonicalIdentifier,
    },
    Generated {
        owner: PersistentTypeId,
        role: GeneratedEnumVariantRole,
    },
}

impl EnumVariantIdentityKey {
    pub const fn source_owner(&self) -> Option<NominalDeclarationOwner> {
        match &self.0 {
            EnumVariantIdentityKeyKind::Source { owner, .. } => Some(*owner),
            EnumVariantIdentityKeyKind::Generated { .. } => None,
        }
    }

    pub const fn source_name(&self) -> Option<&CanonicalIdentifier> {
        match &self.0 {
            EnumVariantIdentityKeyKind::Source { name, .. } => Some(name),
            EnumVariantIdentityKeyKind::Generated { .. } => None,
        }
    }

    pub fn source(
        owner: &SourceDeclarationKey,
        name: CanonicalIdentifier,
    ) -> Result<Self, EnumVariantIdentityError> {
        if owner.declaration_kind() != SourceDeclarationKind::Enum {
            return Err(EnumVariantIdentityError::ExpectedSourceEnum);
        }
        let owner = source_nominal_owner(owner)?;
        Ok(Self(EnumVariantIdentityKeyKind::Source { owner, name }))
    }

    pub fn generated(
        owner: &GeneratedNominalKey,
        role: GeneratedEnumVariantRole,
    ) -> Result<Self, EnumVariantIdentityError> {
        if !generated_role_matches(owner, role) {
            return Err(EnumVariantIdentityError::GeneratedRoleMismatch);
        }
        let owner = PersistentTypeId::from_generated_key(owner)
            .map_err(EnumVariantIdentityError::GeneratedNominal)?;
        Ok(Self(EnumVariantIdentityKeyKind::Generated { owner, role }))
    }
}

impl WireEncode for EnumVariantIdentityKey {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match &self.0 {
            EnumVariantIdentityKeyKind::Source { owner, name } => {
                encode_two_value_sum(encoder, 1, owner, name)
            }
            EnumVariantIdentityKeyKind::Generated { owner, role } => {
                encode_two_value_sum(encoder, 2, owner, role)
            }
        }
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum EnumVariantFieldSelector {
    Named(CanonicalIdentifier),
    Positional { declaration_index: u32 },
}

impl WireEncode for EnumVariantFieldSelector {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::Named(name) => encode_value_sum(encoder, 1, name),
            Self::Positional { declaration_index } => {
                encoder.map(2)?;
                encode_tag(encoder, 2)?;
                encoder.field(1)?;
                encoder.unsigned(u64::from(*declaration_index))
            }
        }
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct EnumVariantFieldKey {
    variant: PersistentEnumVariantId,
    selector: EnumVariantFieldSelector,
}

impl EnumVariantFieldKey {
    pub const fn new(variant: PersistentEnumVariantId, selector: EnumVariantFieldSelector) -> Self {
        Self { variant, selector }
    }

    pub const fn variant(&self) -> PersistentEnumVariantId {
        self.variant
    }

    pub fn selector(&self) -> &EnumVariantFieldSelector {
        &self.selector
    }
}

impl WireEncode for EnumVariantFieldKey {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        self.variant.encode(encoder)?;
        encoder.field(2)?;
        self.selector.encode(encoder)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EnumVariantIdentityError {
    ExpectedSourceEnum,
    GeneratedRoleMismatch,
    SourceDeclaration(SourceDeclarationIdentityError),
    GeneratedNominal(GeneratedNominalIdentityError),
    Hash(HashError),
}

impl fmt::Display for EnumVariantIdentityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ExpectedSourceEnum => {
                formatter.write_str("source enum variant owner must be a source enum")
            }
            Self::GeneratedRoleMismatch => {
                formatter.write_str("generated enum variant role does not match its nominal owner")
            }
            Self::SourceDeclaration(error) => error.fmt(formatter),
            Self::GeneratedNominal(error) => error.fmt(formatter),
            Self::Hash(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for EnumVariantIdentityError {}

impl PersistentEnumVariantId {
    pub fn from_key(key: &EnumVariantIdentityKey) -> Result<Self, EnumVariantIdentityError> {
        derive_persistent_id("scoop-enum-variant-id-v1", key)
            .map_err(EnumVariantIdentityError::Hash)
    }
}

impl PersistentEnumVariantFieldId {
    pub fn from_key(key: &EnumVariantFieldKey) -> Result<Self, HashError> {
        derive_persistent_id("scoop-enum-variant-field-id-v1", key)
    }
}

fn source_nominal_owner(
    key: &SourceDeclarationKey,
) -> Result<NominalDeclarationOwner, EnumVariantIdentityError> {
    NominalDeclarationOwner::from_source_declaration(key)
        .map_err(EnumVariantIdentityError::SourceDeclaration)
}

fn generated_role_matches(owner: &GeneratedNominalKey, role: GeneratedEnumVariantRole) -> bool {
    matches!(
        (owner, role),
        (
            GeneratedNominalKey::CoroutineStep { .. },
            GeneratedEnumVariantRole::CoroutineStepCompleted
                | GeneratedEnumVariantRole::CoroutineStepSuspended
        ) | (
            GeneratedNominalKey::CoroutineSlot { .. },
            GeneratedEnumVariantRole::CoroutineSlotEmpty
                | GeneratedEnumVariantRole::CoroutineSlotValue
        )
    )
}

fn encode_tag(encoder: &mut Encoder, tag: u64) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.field(0)?;
    encoder.unsigned(tag)
}

fn encode_value_sum(
    encoder: &mut Encoder,
    tag: u64,
    value: &impl WireEncode,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.map(2)?;
    encode_tag(encoder, tag)?;
    encoder.field(1)?;
    value.encode(encoder)
}

fn encode_two_value_sum(
    encoder: &mut Encoder,
    tag: u64,
    first: &impl WireEncode,
    second: &impl WireEncode,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.map(3)?;
    encode_tag(encoder, tag)?;
    encoder.field(1)?;
    first.encode(encoder)?;
    encoder.field(2)?;
    second.encode(encoder)
}

#[cfg(test)]
mod tests {
    use scoop_wire::encode;

    use super::{
        EnumVariantFieldKey, EnumVariantFieldSelector, EnumVariantIdentityError,
        EnumVariantIdentityKey, GeneratedEnumVariantRole,
    };
    use crate::{
        ConeIdentity, GeneratedNominalKey, PersistentEnumVariantFieldId, PersistentEnumVariantId,
        PersistentExactTypeId,
    };

    #[test]
    fn generated_variant_and_positional_field_have_fixed_identity() {
        let exact = PersistentExactTypeId(ConeIdentity::CORE.0);
        let owner = GeneratedNominalKey::CoroutineStep { result: exact };
        let variant_key = EnumVariantIdentityKey::generated(
            &owner,
            GeneratedEnumVariantRole::CoroutineStepCompleted,
        )
        .unwrap();
        assert_eq!(
            hex(&encode(&variant_key).unwrap()),
            "a30002015820d08341f11b51ec68c41af0c662986773b16169ccfe7bb399efe456d1c1a8d7f30201"
        );
        let variant = PersistentEnumVariantId::from_key(&variant_key).unwrap();
        assert_eq!(
            variant.to_string(),
            "76b87b4b32938bd3036ec1f44db738ca045b77d4bdd51f03d76453d7a3402633"
        );

        let field_key = EnumVariantFieldKey::new(
            variant,
            EnumVariantFieldSelector::Positional {
                declaration_index: 0,
            },
        );
        assert_eq!(
            PersistentEnumVariantFieldId::from_key(&field_key)
                .unwrap()
                .to_string(),
            "6532d310d3b96912d39c8f48f1e857d52fc132b60b3bbbeb03100b10fd9d4568"
        );
    }

    #[test]
    fn source_variant_constructor_enforces_enum_owner() {
        let enum_owner = source_nominal(crate::SourceNominalKind::Enum);
        let class_owner = source_nominal(crate::SourceNominalKind::Class);
        let name = crate::CanonicalIdentifier::new("Value").unwrap();
        assert!(EnumVariantIdentityKey::source(&enum_owner, name.clone()).is_ok());
        assert_eq!(
            EnumVariantIdentityKey::source(&class_owner, name),
            Err(EnumVariantIdentityError::ExpectedSourceEnum)
        );
    }

    #[test]
    fn source_variant_and_fields_have_fixed_identity() {
        let owner = source_nominal(crate::SourceNominalKind::Enum);
        let variant_key = EnumVariantIdentityKey::source(
            &owner,
            crate::CanonicalIdentifier::new("Value").unwrap(),
        )
        .unwrap();
        let variant = PersistentEnumVariantId::from_key(&variant_key).unwrap();
        let named_key = EnumVariantFieldKey::new(
            variant,
            EnumVariantFieldSelector::Named(crate::CanonicalIdentifier::new("payload").unwrap()),
        );
        let positional_key = EnumVariantFieldKey::new(
            variant,
            EnumVariantFieldSelector::Positional {
                declaration_index: 0,
            },
        );

        assert_eq!(
            hex(&encode(&variant_key).unwrap()),
            "a3000101a20001015820bf837968fb5186f42afc6b58f6ffab652a39ac12b50914e44ca5339e50d7e55a026556616c7565"
        );
        assert_eq!(
            variant.to_string(),
            "c4e60df44cc8994b93bdc3cac596f9998a48296caf9af964860254a4b89a5e37"
        );
        assert_eq!(
            hex(&encode(&named_key).unwrap()),
            "a2015820c4e60df44cc8994b93bdc3cac596f9998a48296caf9af964860254a4b89a5e3702a2000101677061796c6f6164"
        );
        assert_eq!(
            PersistentEnumVariantFieldId::from_key(&named_key)
                .unwrap()
                .to_string(),
            "b776650dc44cfc9b08992c20915de3e4bb058ae718744f5ae0cad7e078efb6b8"
        );
        assert_eq!(
            hex(&encode(&positional_key).unwrap()),
            "a2015820c4e60df44cc8994b93bdc3cac596f9998a48296caf9af964860254a4b89a5e3702a200020100"
        );
        assert_eq!(
            PersistentEnumVariantFieldId::from_key(&positional_key)
                .unwrap()
                .to_string(),
            "fa87274475a049ae232d3b2846013f3f20575cd8425575633d64ba03613c3c9c"
        );
    }

    #[test]
    fn generated_variant_constructor_enforces_owner_matrix() {
        let exact = PersistentExactTypeId(ConeIdentity::CORE.0);
        let owner = GeneratedNominalKey::BoxedValue { payload: exact };
        assert_eq!(
            EnumVariantIdentityKey::generated(&owner, GeneratedEnumVariantRole::CoroutineSlotEmpty,),
            Err(EnumVariantIdentityError::GeneratedRoleMismatch)
        );
    }

    fn source_nominal(kind: crate::SourceNominalKind) -> crate::SourceDeclarationKey {
        crate::SourceDeclarationKey::nominal(
            crate::SourceDeclarationSite::new(
                ConeIdentity::CORE,
                crate::PackagePath::from_segments(vec![
                    crate::CanonicalIdentifier::new("test").unwrap(),
                ]),
                crate::DefinitionOwnerChain::top_level(),
                crate::DeclarationScope::ConeWide,
            )
            .unwrap(),
            crate::CanonicalIdentifier::new("Owner").unwrap(),
            kind,
            0,
        )
    }

    fn hex(bytes: &[u8]) -> String {
        bytes.iter().map(|byte| format!("{byte:02x}")).collect()
    }
}
