use std::fmt;

use scoop_wire::{Encoder, HashError, WireEncodeV1};

use super::{
    GeneratedNominalIdentityError, GeneratedNominalKeyV1, NominalDeclarationOwnerV1,
    SourceDeclarationIdentityError, SourceDeclarationKeyV1, SourceDeclarationKindV1,
};
use crate::ids::derive_persistent_id;
use crate::{
    CanonicalIdentifier, PersistentEnumVariantFieldId, PersistentEnumVariantId, PersistentTypeId,
};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum GeneratedEnumVariantRoleV1 {
    CoroutineStepCompleted,
    CoroutineStepSuspended,
    CoroutineSlotEmpty,
    CoroutineSlotValue,
}

impl WireEncodeV1 for GeneratedEnumVariantRoleV1 {
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
pub struct EnumVariantIdentityKeyV1(EnumVariantIdentityKeyKindV1);

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
enum EnumVariantIdentityKeyKindV1 {
    Source {
        owner: NominalDeclarationOwnerV1,
        name: CanonicalIdentifier,
    },
    Generated {
        owner: PersistentTypeId,
        role: GeneratedEnumVariantRoleV1,
    },
}

impl EnumVariantIdentityKeyV1 {
    pub fn source(
        owner: &SourceDeclarationKeyV1,
        name: CanonicalIdentifier,
    ) -> Result<Self, EnumVariantIdentityError> {
        if owner.declaration_kind() != SourceDeclarationKindV1::Enum {
            return Err(EnumVariantIdentityError::ExpectedSourceEnum);
        }
        let owner = source_nominal_owner(owner)?;
        Ok(Self(EnumVariantIdentityKeyKindV1::Source { owner, name }))
    }

    pub fn generated(
        owner: &GeneratedNominalKeyV1,
        role: GeneratedEnumVariantRoleV1,
    ) -> Result<Self, EnumVariantIdentityError> {
        if !generated_role_matches(owner, role) {
            return Err(EnumVariantIdentityError::GeneratedRoleMismatch);
        }
        let owner = PersistentTypeId::from_generated_key(owner)
            .map_err(EnumVariantIdentityError::GeneratedNominal)?;
        Ok(Self(EnumVariantIdentityKeyKindV1::Generated {
            owner,
            role,
        }))
    }
}

impl WireEncodeV1 for EnumVariantIdentityKeyV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match &self.0 {
            EnumVariantIdentityKeyKindV1::Source { owner, name } => {
                encode_two_value_sum(encoder, 1, owner, name)
            }
            EnumVariantIdentityKeyKindV1::Generated { owner, role } => {
                encode_two_value_sum(encoder, 2, owner, role)
            }
        }
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum EnumVariantFieldSelectorV1 {
    Named(CanonicalIdentifier),
    Positional { declaration_index: u32 },
}

impl WireEncodeV1 for EnumVariantFieldSelectorV1 {
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
pub struct EnumVariantFieldKeyV1 {
    variant: PersistentEnumVariantId,
    selector: EnumVariantFieldSelectorV1,
}

impl EnumVariantFieldKeyV1 {
    pub const fn new(
        variant: PersistentEnumVariantId,
        selector: EnumVariantFieldSelectorV1,
    ) -> Self {
        Self { variant, selector }
    }

    pub const fn variant(&self) -> PersistentEnumVariantId {
        self.variant
    }

    pub fn selector(&self) -> &EnumVariantFieldSelectorV1 {
        &self.selector
    }
}

impl WireEncodeV1 for EnumVariantFieldKeyV1 {
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
    pub fn from_key(key: &EnumVariantIdentityKeyV1) -> Result<Self, EnumVariantIdentityError> {
        derive_persistent_id("scoop-enum-variant-id-v1", key)
            .map_err(EnumVariantIdentityError::Hash)
    }
}

impl PersistentEnumVariantFieldId {
    pub fn from_key(key: &EnumVariantFieldKeyV1) -> Result<Self, HashError> {
        derive_persistent_id("scoop-enum-variant-field-id-v1", key)
    }
}

fn source_nominal_owner(
    key: &SourceDeclarationKeyV1,
) -> Result<NominalDeclarationOwnerV1, EnumVariantIdentityError> {
    NominalDeclarationOwnerV1::from_source_declaration(key)
        .map_err(EnumVariantIdentityError::SourceDeclaration)
}

fn generated_role_matches(owner: &GeneratedNominalKeyV1, role: GeneratedEnumVariantRoleV1) -> bool {
    matches!(
        (owner, role),
        (
            GeneratedNominalKeyV1::CoroutineStep { .. },
            GeneratedEnumVariantRoleV1::CoroutineStepCompleted
                | GeneratedEnumVariantRoleV1::CoroutineStepSuspended
        ) | (
            GeneratedNominalKeyV1::CoroutineSlot { .. },
            GeneratedEnumVariantRoleV1::CoroutineSlotEmpty
                | GeneratedEnumVariantRoleV1::CoroutineSlotValue
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
    value: &impl WireEncodeV1,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.map(2)?;
    encode_tag(encoder, tag)?;
    encoder.field(1)?;
    value.encode(encoder)
}

fn encode_two_value_sum(
    encoder: &mut Encoder,
    tag: u64,
    first: &impl WireEncodeV1,
    second: &impl WireEncodeV1,
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
        EnumVariantFieldKeyV1, EnumVariantFieldSelectorV1, EnumVariantIdentityError,
        EnumVariantIdentityKeyV1, GeneratedEnumVariantRoleV1,
    };
    use crate::{
        ConeIdentity, GeneratedNominalKeyV1, PersistentEnumVariantFieldId, PersistentEnumVariantId,
        PersistentExactTypeId,
    };

    #[test]
    fn generated_variant_and_positional_field_have_fixed_identity() {
        let exact = PersistentExactTypeId(ConeIdentity::CORE.0);
        let owner = GeneratedNominalKeyV1::CoroutineStep { result: exact };
        let variant_key = EnumVariantIdentityKeyV1::generated(
            &owner,
            GeneratedEnumVariantRoleV1::CoroutineStepCompleted,
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

        let field_key = EnumVariantFieldKeyV1::new(
            variant,
            EnumVariantFieldSelectorV1::Positional {
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
        let enum_owner = source_nominal(crate::SourceNominalKindV1::Enum);
        let class_owner = source_nominal(crate::SourceNominalKindV1::Class);
        let name = crate::CanonicalIdentifier::new("Value").unwrap();
        assert!(EnumVariantIdentityKeyV1::source(&enum_owner, name.clone()).is_ok());
        assert_eq!(
            EnumVariantIdentityKeyV1::source(&class_owner, name),
            Err(EnumVariantIdentityError::ExpectedSourceEnum)
        );
    }

    #[test]
    fn generated_variant_constructor_enforces_owner_matrix() {
        let exact = PersistentExactTypeId(ConeIdentity::CORE.0);
        let owner = GeneratedNominalKeyV1::BoxedValue { payload: exact };
        assert_eq!(
            EnumVariantIdentityKeyV1::generated(
                &owner,
                GeneratedEnumVariantRoleV1::CoroutineSlotEmpty,
            ),
            Err(EnumVariantIdentityError::GeneratedRoleMismatch)
        );
    }

    fn source_nominal(kind: crate::SourceNominalKindV1) -> crate::SourceDeclarationKeyV1 {
        crate::SourceDeclarationKeyV1::nominal(
            crate::SourceDeclarationSiteV1::new(
                ConeIdentity::CORE,
                crate::PackagePath::from_segments(vec![
                    crate::CanonicalIdentifier::new("test").unwrap(),
                ]),
                crate::DefinitionOwnerChainV1::top_level(),
                crate::DeclarationScopeV1::ConeWide,
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
