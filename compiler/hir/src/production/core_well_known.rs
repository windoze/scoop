use std::fmt;

use scoop_identity::{
    ConeIdentity, DeclarationName, DeclarationScope, DecodedPersistentId, ExactTypeKey,
    PersistentExactTypeId, PersistentTypeId, SourceDeclarationKind,
};
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError, WireErrorKind};

use crate::{CanonicalHirFoundation, ExportHir, HirNominalIdentity, Type, ValidatedHirFoundation};

/// Runtime-visible core capabilities whose semantic owner is established in
/// target-independent Export HIR. Target layout is attached by LIR later.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeCoreCapabilityV1 {
    String {
        source_type: PersistentTypeId,
        exact_type: PersistentExactTypeId,
    },
}

impl RuntimeCoreCapabilityV1 {
    pub fn string_from_core_export(
        export: &ExportHir,
    ) -> Result<Self, RuntimeCoreCapabilityBuildError> {
        if export.cone != ConeIdentity::CORE {
            return Err(RuntimeCoreCapabilityBuildError::NotCore(export.cone));
        }
        let class = export.intrinsic_type_core.string;
        let HirNominalIdentity::Source(source_identity) = &export.nominal_identities[class] else {
            return Err(RuntimeCoreCapabilityBuildError::StringSourceNotSource);
        };
        let source_type = source_identity
            .concrete_id()
            .ok_or(RuntimeCoreCapabilityBuildError::StringSourceIsGeneric)?;
        validate_string_declaration(source_identity.declaration())
            .map_err(|_| RuntimeCoreCapabilityBuildError::InvalidStringDeclaration)?;
        if !matches!(&export.types[export.string], Type::String) {
            return Err(RuntimeCoreCapabilityBuildError::InvalidStringTypeSlot);
        }
        let exact_record = export
            .type_identities
            .get(export.string)
            .and_then(|identity| identity.exact())
            .ok_or(RuntimeCoreCapabilityBuildError::StringTypeIsOpen)?;
        if exact_record.key() != &ExactTypeKey::Nominal(source_type) {
            return Err(RuntimeCoreCapabilityBuildError::StringExactTypeMismatch);
        }
        Ok(Self::String {
            source_type,
            exact_type: exact_record.id(),
        })
    }

    pub const fn source_type(self) -> PersistentTypeId {
        match self {
            Self::String { source_type, .. } => source_type,
        }
    }

    pub const fn exact_type(self) -> PersistentExactTypeId {
        match self {
            Self::String { exact_type, .. } => exact_type,
        }
    }
}

impl WireEncode for RuntimeCoreCapabilityV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::String {
                source_type,
                exact_type,
            } => {
                encoder.map(3)?;
                encoder.field(0)?;
                encoder.unsigned(1)?;
                encoder.field(1)?;
                source_type.encode(encoder)?;
                encoder.field(2)?;
                exact_type.encode(encoder)
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DecodedRuntimeCoreCapabilityV1 {
    String {
        source_type: DecodedPersistentId<PersistentTypeId>,
        exact_type: DecodedPersistentId<PersistentExactTypeId>,
    },
}

impl DecodedRuntimeCoreCapabilityV1 {
    pub fn validate(
        self,
        foundation: &ValidatedHirFoundation,
    ) -> Result<RuntimeCoreCapabilityV1, RuntimeCoreCapabilityValidationError> {
        self.validate_against(foundation.canonical())
    }

    pub(super) fn validate_against(
        self,
        foundation: &CanonicalHirFoundation,
    ) -> Result<RuntimeCoreCapabilityV1, RuntimeCoreCapabilityValidationError> {
        match self {
            Self::String {
                source_type,
                exact_type,
            } => {
                let (source_type, declaration) = foundation
                    .source_type_by_bytes(source_type.as_array())
                    .ok_or(RuntimeCoreCapabilityValidationError::UnknownStringSource(
                        *source_type.as_array(),
                    ))?;
                validate_string_declaration(declaration).map_err(|_| {
                    RuntimeCoreCapabilityValidationError::InvalidStringDeclaration(source_type)
                })?;
                let (exact_type, exact_key) = foundation
                    .exact_type_by_bytes(exact_type.as_array())
                    .ok_or(RuntimeCoreCapabilityValidationError::UnknownStringExact(
                        *exact_type.as_array(),
                    ))?;
                if exact_key != &ExactTypeKey::Nominal(source_type) {
                    return Err(
                        RuntimeCoreCapabilityValidationError::StringExactTypeMismatch {
                            source_type,
                            exact_type,
                        },
                    );
                }
                Ok(RuntimeCoreCapabilityV1::String {
                    source_type,
                    exact_type,
                })
            }
        }
    }
}

impl WireEncode for DecodedRuntimeCoreCapabilityV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::String {
                source_type,
                exact_type,
            } => {
                encoder.map(3)?;
                encoder.field(0)?;
                encoder.unsigned(1)?;
                encoder.field(1)?;
                source_type.encode(encoder)?;
                encoder.field(2)?;
                exact_type.encode(encoder)
            }
        }
    }
}

impl WireDecode for DecodedRuntimeCoreCapabilityV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        let fields = decoder.map()?;
        if fields == 0 {
            return Err(wire_error(
                decoder,
                WireErrorKind::MissingField { field: 0 },
            ));
        }
        let tag = decoder.field(0, Decoder::unsigned)?;
        match tag {
            1 => {
                require_sum_length(decoder, fields, 3)?;
                Ok(Self::String {
                    source_type: decoder.field(1, DecodedPersistentId::decode)?,
                    exact_type: decoder.field(2, DecodedPersistentId::decode)?,
                })
            }
            tag => Err(wire_error(decoder, WireErrorKind::UnknownTag { tag })),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeCoreCapabilityBuildError {
    NotCore(ConeIdentity),
    StringSourceNotSource,
    StringSourceIsGeneric,
    InvalidStringDeclaration,
    InvalidStringTypeSlot,
    StringTypeIsOpen,
    StringExactTypeMismatch,
}

impl fmt::Display for RuntimeCoreCapabilityBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotCore(cone) => write!(
                formatter,
                "runtime core capability requires the reserved core Cone, found {cone}"
            ),
            Self::StringSourceNotSource => {
                formatter.write_str("core String intrinsic owner is not a source declaration")
            }
            Self::StringSourceIsGeneric => {
                formatter.write_str("core String intrinsic owner is generic")
            }
            Self::InvalidStringDeclaration => {
                formatter.write_str("core String source declaration has an invalid identity shape")
            }
            Self::InvalidStringTypeSlot => {
                formatter.write_str("core String type slot does not contain the String type")
            }
            Self::StringTypeIsOpen => formatter.write_str("core String type is open"),
            Self::StringExactTypeMismatch => formatter
                .write_str("core String exact type does not name its intrinsic source declaration"),
        }
    }
}

impl std::error::Error for RuntimeCoreCapabilityBuildError {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeCoreCapabilityValidationError {
    UnknownStringSource([u8; 32]),
    UnknownStringExact([u8; 32]),
    InvalidStringDeclaration(PersistentTypeId),
    StringExactTypeMismatch {
        source_type: PersistentTypeId,
        exact_type: PersistentExactTypeId,
    },
}

impl fmt::Display for RuntimeCoreCapabilityValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownStringSource(id) => write!(
                formatter,
                "runtime String source type {} is absent from the HIR foundation",
                Hex(id)
            ),
            Self::UnknownStringExact(id) => write!(
                formatter,
                "runtime String exact type {} is absent from the HIR foundation",
                Hex(id)
            ),
            Self::InvalidStringDeclaration(id) => write!(
                formatter,
                "runtime String source type {id} is not the reserved core String class"
            ),
            Self::StringExactTypeMismatch {
                source_type,
                exact_type,
            } => write!(
                formatter,
                "runtime String exact type {exact_type} is not the nominal exact identity of {source_type}"
            ),
        }
    }
}

impl std::error::Error for RuntimeCoreCapabilityValidationError {}

fn validate_string_declaration(
    declaration: &scoop_identity::SourceDeclarationKey,
) -> Result<(), ()> {
    let valid = declaration.origin() == ConeIdentity::CORE
        && declaration.package().is_root()
        && declaration.owners().owners().is_empty()
        && declaration.scope() == &DeclarationScope::ConeWide
        && declaration.declaration_kind() == SourceDeclarationKind::Class
        && matches!(
            declaration.name(),
            DeclarationName::Named(name) if name.as_str() == "String"
        )
        && declaration.duplicate_signature().type_parameter_count() == 0;
    valid.then_some(()).ok_or(())
}

fn require_sum_length(
    decoder: &Decoder<'_, '_>,
    actual: u64,
    expected: u64,
) -> Result<(), WireError> {
    if actual == expected {
        Ok(())
    } else {
        Err(wire_error(
            decoder,
            WireErrorKind::InvalidLength { expected, actual },
        ))
    }
}

fn wire_error(decoder: &Decoder<'_, '_>, kind: WireErrorKind) -> WireError {
    WireError::new(kind, decoder.path().clone(), Some(decoder.position()))
}

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
        CanonicalIdentifier, CborIdentityRecord, DeclarationScope, DefinitionOwnerChain,
        ExactTypeKey, PackagePath, PersistentExactTypeId, PersistentTypeId, SourceDeclarationKey,
        SourceDeclarationSite, SourceNominalKind,
    };
    use scoop_wire::{DecodeLimits, decode_canonical, encode};

    use super::{
        DecodedRuntimeCoreCapabilityV1, RuntimeCoreCapabilityV1,
        RuntimeCoreCapabilityValidationError,
    };
    use crate::CanonicalHirFoundation;

    #[test]
    fn string_capability_has_a_fixed_wire_vector_and_validates_the_nominal_relation() {
        let fixture = fixture("String", scoop_identity::ConeIdentity::CORE);
        let bytes = encode(&fixture.capability).unwrap();
        assert_eq!(
            hex(&bytes),
            "a300010158200252c865acc9bf0786a7b7e2b1ca777f76ae038ab51bb159b9a6ad961aad6097025820ad3ae7a719e82101f547257b8a8ac185f05531c14504be81962250566a3ee868"
        );

        assert_eq!(
            decode(&fixture.capability)
                .validate_against(&fixture.foundation)
                .unwrap(),
            fixture.capability
        );
    }

    #[test]
    fn string_capability_reader_rejects_unknown_missing_and_extra_sum_fields() {
        for bytes in [
            vec![0xa1, 0x00, 0x02],
            vec![0xa0],
            vec![0xa4, 0x00, 0x01, 0x01, 0x40, 0x02, 0x40, 0x03, 0x00],
        ] {
            assert!(
                decode_canonical::<DecodedRuntimeCoreCapabilityV1>(&bytes, DecodeLimits::default())
                    .is_err()
            );
        }
    }

    #[test]
    fn string_capability_rejects_unknown_typed_identities() {
        let primary = fixture("String", scoop_identity::ConeIdentity::CORE);
        let unknown = fixture("Other", scoop_identity::ConeIdentity::CORE);
        let unknown_source = RuntimeCoreCapabilityV1::String {
            source_type: unknown.capability.source_type(),
            exact_type: primary.capability.exact_type(),
        };
        let unknown_source_id = *unknown_source.source_type().as_array();
        assert_eq!(
            decode(&unknown_source).validate_against(&primary.foundation),
            Err(RuntimeCoreCapabilityValidationError::UnknownStringSource(
                unknown_source_id
            ))
        );

        let unknown_exact = RuntimeCoreCapabilityV1::String {
            source_type: primary.capability.source_type(),
            exact_type: unknown.capability.exact_type(),
        };
        let unknown_exact_id = *unknown_exact.exact_type().as_array();
        assert_eq!(
            decode(&unknown_exact).validate_against(&primary.foundation),
            Err(RuntimeCoreCapabilityValidationError::UnknownStringExact(
                unknown_exact_id
            ))
        );
    }

    #[test]
    fn string_capability_rejects_name_origin_and_exact_relation_substitution() {
        for fixture in [
            fixture("NotString", scoop_identity::ConeIdentity::CORE),
            fixture("String", scoop_identity::ConeIdentity::SINGLE_FILE),
        ] {
            let source_type = fixture.capability.source_type();
            assert_eq!(
                decode(&fixture.capability).validate_against(&fixture.foundation),
                Err(RuntimeCoreCapabilityValidationError::InvalidStringDeclaration(source_type))
            );
        }

        let primary = fixture("String", scoop_identity::ConeIdentity::CORE);
        let substitute = fixture("Other", scoop_identity::ConeIdentity::CORE);
        let mut foundation = primary.foundation;
        foundation
            .set_types(vec![primary.source_record, substitute.source_record])
            .unwrap();
        foundation
            .set_exact_types(vec![primary.exact_record, substitute.exact_record])
            .unwrap();
        let capability = RuntimeCoreCapabilityV1::String {
            source_type: primary.capability.source_type(),
            exact_type: substitute.capability.exact_type(),
        };
        assert_eq!(
            decode(&capability).validate_against(&foundation),
            Err(
                RuntimeCoreCapabilityValidationError::StringExactTypeMismatch {
                    source_type: capability.source_type(),
                    exact_type: capability.exact_type(),
                }
            )
        );
    }

    fn decode(capability: &RuntimeCoreCapabilityV1) -> DecodedRuntimeCoreCapabilityV1 {
        let bytes = encode(capability).unwrap();
        decode_canonical(&bytes, DecodeLimits::default()).unwrap()
    }

    struct Fixture {
        foundation: CanonicalHirFoundation,
        capability: RuntimeCoreCapabilityV1,
        source_record: CborIdentityRecord<PersistentTypeId, SourceDeclarationKey>,
        exact_record: CborIdentityRecord<PersistentExactTypeId, ExactTypeKey>,
    }

    fn fixture(name: &str, cone: scoop_identity::ConeIdentity) -> Fixture {
        let declaration = SourceDeclarationKey::nominal(
            SourceDeclarationSite::new(
                cone,
                PackagePath::root(),
                DefinitionOwnerChain::top_level(),
                DeclarationScope::ConeWide,
            )
            .unwrap(),
            CanonicalIdentifier::new(name).unwrap(),
            SourceNominalKind::Class,
            0,
        );
        let source_record = CborIdentityRecord::from_key(declaration).unwrap();
        let exact_record =
            CborIdentityRecord::from_key(ExactTypeKey::Nominal(source_record.id())).unwrap();
        let capability = RuntimeCoreCapabilityV1::String {
            source_type: source_record.id(),
            exact_type: exact_record.id(),
        };
        let mut foundation = CanonicalHirFoundation::empty();
        foundation.set_types(vec![source_record.clone()]).unwrap();
        foundation
            .set_exact_types(vec![exact_record.clone()])
            .unwrap();
        Fixture {
            foundation,
            capability,
            source_record,
            exact_record,
        }
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
