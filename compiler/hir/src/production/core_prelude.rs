use std::fmt;

use scoop_identity::{
    BindableEntity, BindingNamespace, BindingRole, ConeIdentity, DeclarationName,
    DecodedPersistentId, EnumVariantFieldSelector, NominalDeclarationOwner,
    PersistentEnumVariantFieldId, PersistentEnumVariantId, SourceDeclarationKind,
};
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError};

use super::{
    CanonicalDirectPublicSurfaceV1, DecodedCanonicalDirectPublicSurfaceV1,
    DirectPublicSurfaceBuildError, DirectPublicSurfaceValidationError,
};
use crate::{CanonicalHirFoundation, ExportHir, ValidatedHirFoundation};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CorePreludeSnapshotV1 {
    pub(super) ordinary_bindings: CanonicalDirectPublicSurfaceV1,
    pub(super) option_some: PersistentEnumVariantId,
    pub(super) option_some_payload: PersistentEnumVariantFieldId,
    pub(super) option_none: PersistentEnumVariantId,
}

impl CorePreludeSnapshotV1 {
    pub fn from_core_export(export: &ExportHir) -> Result<Self, CorePreludeSnapshotBuildError> {
        if export.cone != ConeIdentity::CORE {
            return Err(CorePreludeSnapshotBuildError::NotCore(export.cone));
        }
        let ordinary_bindings = CanonicalDirectPublicSurfaceV1::from_export_hir(export)
            .map_err(CorePreludeSnapshotBuildError::DirectSurface)?;
        let option_some = export.enum_member_identities[export.core_protocols.option.some()].id();
        let option_some_payload =
            export.enum_member_identities[export.core_protocols.option.some_payload()].id();
        let option_none = export.enum_member_identities[export.core_protocols.option.none()].id();
        Ok(Self {
            ordinary_bindings,
            option_some,
            option_some_payload,
            option_none,
        })
    }

    pub const fn ordinary_bindings(&self) -> &CanonicalDirectPublicSurfaceV1 {
        &self.ordinary_bindings
    }

    pub const fn option_some(&self) -> PersistentEnumVariantId {
        self.option_some
    }

    pub const fn option_some_payload(&self) -> PersistentEnumVariantFieldId {
        self.option_some_payload
    }

    pub const fn option_none(&self) -> PersistentEnumVariantId {
        self.option_none
    }
}

impl WireEncode for CorePreludeSnapshotV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(4)?;
        encoder.field(1)?;
        self.ordinary_bindings.encode(encoder)?;
        encoder.field(2)?;
        self.option_some.encode(encoder)?;
        encoder.field(3)?;
        self.option_some_payload.encode(encoder)?;
        encoder.field(4)?;
        self.option_none.encode(encoder)
    }
}

#[derive(Debug)]
pub struct DecodedCorePreludeSnapshotV1 {
    ordinary_bindings: DecodedCanonicalDirectPublicSurfaceV1,
    option_some: DecodedPersistentId<PersistentEnumVariantId>,
    option_some_payload: DecodedPersistentId<PersistentEnumVariantFieldId>,
    option_none: DecodedPersistentId<PersistentEnumVariantId>,
}

impl DecodedCorePreludeSnapshotV1 {
    pub fn validate(
        self,
        foundation: &ValidatedHirFoundation,
    ) -> Result<CorePreludeSnapshotV1, CorePreludeSnapshotValidationError> {
        self.validate_against(foundation.canonical())
    }

    pub(super) fn validate_against(
        self,
        foundation: &CanonicalHirFoundation,
    ) -> Result<CorePreludeSnapshotV1, CorePreludeSnapshotValidationError> {
        let ordinary_bindings = self
            .ordinary_bindings
            .validate_against(foundation)
            .map_err(CorePreludeSnapshotValidationError::DirectSurface)?;
        let (option_some, some_key) = foundation
            .enum_variant_by_bytes(self.option_some.as_array())
            .ok_or(CorePreludeSnapshotValidationError::UnknownSome(
                *self.option_some.as_array(),
            ))?;
        let (option_none, none_key) = foundation
            .enum_variant_by_bytes(self.option_none.as_array())
            .ok_or(CorePreludeSnapshotValidationError::UnknownNone(
                *self.option_none.as_array(),
            ))?;
        let (option_some_payload, payload_key) = foundation
            .enum_variant_field_by_bytes(self.option_some_payload.as_array())
            .ok_or(CorePreludeSnapshotValidationError::UnknownSomePayload(
                *self.option_some_payload.as_array(),
            ))?;

        let Some(some_owner) = some_key.source_owner() else {
            return Err(CorePreludeSnapshotValidationError::GeneratedSome);
        };
        let Some(none_owner) = none_key.source_owner() else {
            return Err(CorePreludeSnapshotValidationError::GeneratedNone);
        };
        if some_key.source_name().map(|name| name.as_str()) != Some("Some") {
            return Err(CorePreludeSnapshotValidationError::WrongSomeName);
        }
        if none_key.source_name().map(|name| name.as_str()) != Some("None") {
            return Err(CorePreludeSnapshotValidationError::WrongNoneName);
        }
        if some_owner != none_owner || option_some == option_none {
            return Err(CorePreludeSnapshotValidationError::OptionOwnerMismatch);
        }
        let NominalDeclarationOwner::GenericTemplate(option_owner) = some_owner else {
            return Err(CorePreludeSnapshotValidationError::OptionOwnerNotGeneric);
        };
        let option_declaration = foundation.generic_type_key(option_owner).ok_or(
            CorePreludeSnapshotValidationError::UnknownOptionOwner(option_owner),
        )?;
        if option_declaration.origin() != ConeIdentity::CORE
            || option_declaration.declaration_kind() != SourceDeclarationKind::Enum
            || !matches!(
                option_declaration.name(),
                DeclarationName::Named(name) if name.as_str() == "Option"
            )
            || option_declaration
                .duplicate_signature()
                .type_parameter_count()
                != 1
        {
            return Err(CorePreludeSnapshotValidationError::InvalidOptionOwner(
                option_owner,
            ));
        }
        if payload_key.variant() != option_some
            || payload_key.selector()
                != &(EnumVariantFieldSelector::Positional {
                    declaration_index: 0,
                })
        {
            return Err(CorePreludeSnapshotValidationError::InvalidSomePayload);
        }
        if foundation.enum_variant_field_count(option_some) != 1 {
            return Err(CorePreludeSnapshotValidationError::InvalidSomeFieldCount);
        }
        if foundation.enum_variant_field_count(option_none) != 0 {
            return Err(CorePreludeSnapshotValidationError::InvalidNoneFieldCount);
        }
        let has_option_binding = ordinary_bindings.bindings().iter().any(|binding| {
            let Some(key) = foundation.export_binding_key(*binding) else {
                return false;
            };
            key.exporter() == ConeIdentity::CORE
                && key.namespace() == BindingNamespace::Type
                && key.role() == BindingRole::TypeName
                && key.name().as_str() == "Option"
                && key.target() == BindableEntity::GenericType(option_owner)
        });
        if !has_option_binding {
            return Err(CorePreludeSnapshotValidationError::MissingPublicOptionBinding);
        }

        Ok(CorePreludeSnapshotV1 {
            ordinary_bindings,
            option_some,
            option_some_payload,
            option_none,
        })
    }
}

impl WireEncode for DecodedCorePreludeSnapshotV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(4)?;
        encoder.field(1)?;
        self.ordinary_bindings.encode(encoder)?;
        encoder.field(2)?;
        self.option_some.encode(encoder)?;
        encoder.field(3)?;
        self.option_some_payload.encode(encoder)?;
        encoder.field(4)?;
        self.option_none.encode(encoder)
    }
}

impl WireDecode for DecodedCorePreludeSnapshotV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder.expect_map(4)?;
        Ok(Self {
            ordinary_bindings: decoder.field(1, DecodedCanonicalDirectPublicSurfaceV1::decode)?,
            option_some: decoder.field(2, DecodedPersistentId::decode)?,
            option_some_payload: decoder.field(3, DecodedPersistentId::decode)?,
            option_none: decoder.field(4, DecodedPersistentId::decode)?,
        })
    }
}

#[derive(Debug)]
pub enum CorePreludeSnapshotBuildError {
    NotCore(ConeIdentity),
    DirectSurface(DirectPublicSurfaceBuildError),
}

impl fmt::Display for CorePreludeSnapshotBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotCore(cone) => write!(
                formatter,
                "core prelude snapshot requires the reserved core Cone, found {cone}"
            ),
            Self::DirectSurface(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for CorePreludeSnapshotBuildError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::NotCore(_) => None,
            Self::DirectSurface(error) => Some(error),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CorePreludeSnapshotValidationError {
    DirectSurface(DirectPublicSurfaceValidationError),
    UnknownSome([u8; 32]),
    UnknownSomePayload([u8; 32]),
    UnknownNone([u8; 32]),
    GeneratedSome,
    GeneratedNone,
    WrongSomeName,
    WrongNoneName,
    OptionOwnerMismatch,
    OptionOwnerNotGeneric,
    UnknownOptionOwner(scoop_identity::PersistentGenericTypeId),
    InvalidOptionOwner(scoop_identity::PersistentGenericTypeId),
    InvalidSomePayload,
    InvalidSomeFieldCount,
    InvalidNoneFieldCount,
    MissingPublicOptionBinding,
}

impl fmt::Display for CorePreludeSnapshotValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DirectSurface(error) => error.fmt(formatter),
            Self::UnknownSome(id) => write!(formatter, "unknown core Some variant {}", Hex(id)),
            Self::UnknownSomePayload(id) => {
                write!(formatter, "unknown core Some payload field {}", Hex(id))
            }
            Self::UnknownNone(id) => write!(formatter, "unknown core None variant {}", Hex(id)),
            Self::GeneratedSome => formatter.write_str("core Some variant is generated"),
            Self::GeneratedNone => formatter.write_str("core None variant is generated"),
            Self::WrongSomeName => formatter.write_str("core Some variant has the wrong name"),
            Self::WrongNoneName => formatter.write_str("core None variant has the wrong name"),
            Self::OptionOwnerMismatch => {
                formatter.write_str("core Some and None variants have different Option owners")
            }
            Self::OptionOwnerNotGeneric => {
                formatter.write_str("core Option variant owner is not a generic template")
            }
            Self::UnknownOptionOwner(id) => write!(formatter, "unknown core Option owner {id}"),
            Self::InvalidOptionOwner(id) => write!(
                formatter,
                "core Option owner {id} is not the reserved generic Option enum"
            ),
            Self::InvalidSomePayload => {
                formatter.write_str("core Some payload is not its positional field 0")
            }
            Self::InvalidSomeFieldCount => {
                formatter.write_str("core Some variant does not have exactly one field")
            }
            Self::InvalidNoneFieldCount => {
                formatter.write_str("core None variant unexpectedly has a field")
            }
            Self::MissingPublicOptionBinding => {
                formatter.write_str("core prelude omits the public Option type binding")
            }
        }
    }
}

impl std::error::Error for CorePreludeSnapshotValidationError {}

struct Hex<'a>(&'a [u8; 32]);

impl fmt::Display for Hex<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        for byte in self.0 {
            write!(formatter, "{byte:02x}")?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use scoop_identity::{
        BindingTarget, CanonicalIdentifier, CborIdentityRecord, DeclarationScope,
        DefinitionOwnerChain, EnumVariantFieldKey, EnumVariantFieldSelector,
        EnumVariantIdentityKey, ExportBindingKey, PackagePath, PersistentEnumVariantFieldId,
        PersistentEnumVariantId, PersistentExportBindingId, PersistentGenericTypeId,
        SourceDeclarationKey, SourceDeclarationSite, SourceNominalKind,
    };
    use scoop_wire::{DecodeLimits, decode_canonical, encode};

    use super::{
        CorePreludeSnapshotV1, CorePreludeSnapshotValidationError, DecodedCorePreludeSnapshotV1,
    };
    use crate::{CanonicalDirectPublicSurfaceV1, CanonicalHirFoundation};

    #[test]
    fn snapshot_has_a_fixed_wire_vector_and_validates_all_option_relations() {
        let fixture = fixture();
        let bytes = encode(&fixture.snapshot).unwrap();
        assert_eq!(
            hex(&bytes),
            "a4018158207f51ff5c92b58632d1c34a6bbb2a6a049b7f62f9346af2ff15bc634796879c74025820faf63376f9f38e514ad8979cc6cdeb6a9b86ce537a42088d289fe72f3888e0360358204e898ae8df7bb1bf27b577557433460cd313549ac96a109058dd1499115b5e5b045820dbb5e74c2eb2076e994fea90f134c93ad7ada2ba9a9ef77cb0318c254c935a38"
        );

        let decoded =
            decode_canonical::<DecodedCorePreludeSnapshotV1>(&bytes, DecodeLimits::default())
                .unwrap();
        assert_eq!(
            decoded.validate_against(&fixture.foundation).unwrap(),
            fixture.snapshot
        );
    }

    #[test]
    fn snapshot_reader_rejects_missing_unknown_and_duplicate_fields() {
        let bytes = encode(&fixture().snapshot).unwrap();

        let mut missing = bytes.clone();
        missing[0] = 0xa3;
        assert!(
            decode_canonical::<DecodedCorePreludeSnapshotV1>(&missing, DecodeLimits::default())
                .is_err()
        );

        let mut unknown = bytes.clone();
        let last_field = unknown.len() - 35;
        unknown[last_field] = 0x05;
        assert!(
            decode_canonical::<DecodedCorePreludeSnapshotV1>(&unknown, DecodeLimits::default())
                .is_err()
        );

        let mut duplicate = bytes;
        let last_field = duplicate.len() - 35;
        duplicate[last_field] = 0x03;
        assert!(
            decode_canonical::<DecodedCorePreludeSnapshotV1>(&duplicate, DecodeLimits::default())
                .is_err()
        );
    }

    #[test]
    fn snapshot_rejects_swapped_variants_and_a_missing_public_option_binding() {
        let fixture = fixture();
        let swapped = CorePreludeSnapshotV1 {
            ordinary_bindings: fixture.snapshot.ordinary_bindings.clone(),
            option_some: fixture.snapshot.option_none,
            option_some_payload: fixture.snapshot.option_some_payload,
            option_none: fixture.snapshot.option_some,
        };
        assert_eq!(
            decode(&swapped).validate_against(&fixture.foundation),
            Err(CorePreludeSnapshotValidationError::WrongSomeName)
        );

        let missing_binding = CorePreludeSnapshotV1 {
            ordinary_bindings: CanonicalDirectPublicSurfaceV1::try_new(Vec::new()).unwrap(),
            option_some: fixture.snapshot.option_some,
            option_some_payload: fixture.snapshot.option_some_payload,
            option_none: fixture.snapshot.option_none,
        };
        assert_eq!(
            decode(&missing_binding).validate_against(&fixture.foundation),
            Err(CorePreludeSnapshotValidationError::MissingPublicOptionBinding)
        );
    }

    #[test]
    fn snapshot_rejects_incomplete_some_and_nonempty_none_shapes() {
        let fixture = fixture();
        let extra_some = variant_field(fixture.snapshot.option_some, 1);
        let mut invalid_some = fixture.foundation.clone();
        invalid_some
            .set_enum_variant_fields(vec![fixture.some_payload.clone(), extra_some])
            .unwrap();
        assert_eq!(
            decode(&fixture.snapshot).validate_against(&invalid_some),
            Err(CorePreludeSnapshotValidationError::InvalidSomeFieldCount)
        );

        let none_payload = variant_field(fixture.snapshot.option_none, 0);
        let mut invalid_none = fixture.foundation;
        invalid_none
            .set_enum_variant_fields(vec![fixture.some_payload, none_payload])
            .unwrap();
        assert_eq!(
            decode(&fixture.snapshot).validate_against(&invalid_none),
            Err(CorePreludeSnapshotValidationError::InvalidNoneFieldCount)
        );
    }

    fn decode(snapshot: &CorePreludeSnapshotV1) -> DecodedCorePreludeSnapshotV1 {
        let bytes = encode(snapshot).unwrap();
        decode_canonical(&bytes, DecodeLimits::default()).unwrap()
    }

    #[derive(Clone)]
    struct Fixture {
        foundation: CanonicalHirFoundation,
        snapshot: CorePreludeSnapshotV1,
        some_payload: CborIdentityRecord<PersistentEnumVariantFieldId, EnumVariantFieldKey>,
    }

    fn fixture() -> Fixture {
        let option = SourceDeclarationKey::nominal(
            SourceDeclarationSite::new(
                scoop_identity::ConeIdentity::CORE,
                PackagePath::root(),
                DefinitionOwnerChain::top_level(),
                DeclarationScope::ConeWide,
            )
            .unwrap(),
            CanonicalIdentifier::new("Option").unwrap(),
            SourceNominalKind::Enum,
            1,
        );
        let option_record: CborIdentityRecord<PersistentGenericTypeId, SourceDeclarationKey> =
            CborIdentityRecord::from_key(option.clone()).unwrap();
        let some: CborIdentityRecord<PersistentEnumVariantId, EnumVariantIdentityKey> =
            CborIdentityRecord::from_key(
                EnumVariantIdentityKey::source(&option, CanonicalIdentifier::new("Some").unwrap())
                    .unwrap(),
            )
            .unwrap();
        let none: CborIdentityRecord<PersistentEnumVariantId, EnumVariantIdentityKey> =
            CborIdentityRecord::from_key(
                EnumVariantIdentityKey::source(&option, CanonicalIdentifier::new("None").unwrap())
                    .unwrap(),
            )
            .unwrap();
        let some_payload = variant_field(some.id(), 0);
        let binding: CborIdentityRecord<PersistentExportBindingId, ExportBindingKey> =
            CborIdentityRecord::from_key(ExportBindingKey::new(
                scoop_identity::ConeIdentity::CORE,
                PackagePath::root(),
                CanonicalIdentifier::new("Option").unwrap(),
                BindingTarget::type_name(&option).unwrap(),
            ))
            .unwrap();

        let mut foundation = CanonicalHirFoundation::empty();
        foundation.set_generic_types(vec![option_record]).unwrap();
        foundation
            .set_enum_variants(vec![some.clone(), none.clone()])
            .unwrap();
        foundation
            .set_enum_variant_fields(vec![some_payload.clone()])
            .unwrap();
        foundation
            .set_export_bindings(vec![binding.clone()])
            .unwrap();
        let snapshot = CorePreludeSnapshotV1 {
            ordinary_bindings: CanonicalDirectPublicSurfaceV1::try_new(vec![binding.id()]).unwrap(),
            option_some: some.id(),
            option_some_payload: some_payload.id(),
            option_none: none.id(),
        };
        Fixture {
            foundation,
            snapshot,
            some_payload,
        }
    }

    fn variant_field(
        variant: PersistentEnumVariantId,
        declaration_index: u32,
    ) -> CborIdentityRecord<PersistentEnumVariantFieldId, EnumVariantFieldKey> {
        CborIdentityRecord::from_key(EnumVariantFieldKey::new(
            variant,
            EnumVariantFieldSelector::Positional { declaration_index },
        ))
        .unwrap()
    }

    fn hex(bytes: &[u8]) -> String {
        let mut output = String::with_capacity(bytes.len() * 2);
        for byte in bytes {
            use std::fmt::Write;
            write!(&mut output, "{byte:02x}").unwrap();
        }
        output
    }
}
