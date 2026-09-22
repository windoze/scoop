use scoop_identity::{
    CanonicalIdentifier, CborIdentityRecord, DeclarationScope, DefinitionOwnerChain, ExactTypeKey,
    PackagePath, PersistentExactTypeId, PersistentTypeId, SourceDeclarationKey,
    SourceDeclarationSite, SourceNominalKind,
};
use scoop_wire::{DecodeLimits, decode_canonical, encode};

use super::{
    DecodedRuntimeCoreCapabilityV1, RuntimeCoreCapabilityV1, RuntimeCoreCapabilityValidationError,
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
fn string_capability_rejects_exact_relation_substitution() {
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

#[test]
fn string_role_uses_typed_declarations_independent_of_name_provider_package_and_scope() {
    use scoop_identity::{ConeCoordinate, NormalizedSourcePath, SourceIdentity};
    let provider = ConeCoordinate::new("test", "strings", "1.0.0")
        .unwrap()
        .identity()
        .unwrap();
    let source =
        SourceIdentity::new(provider, NormalizedSourcePath::new("text.scoop").unwrap()).unwrap();
    for scope in [
        DeclarationScope::ConeWide,
        DeclarationScope::SourceScoped(source),
    ] {
        let declaration = SourceDeclarationKey::nominal(
            SourceDeclarationSite::new(
                provider,
                PackagePath::from_segments(vec![CanonicalIdentifier::new("text").unwrap()]),
                DefinitionOwnerChain::top_level(),
                scope,
            )
            .unwrap(),
            CanonicalIdentifier::new("TextValue").unwrap(),
            SourceNominalKind::Class,
            0,
        );
        let fixture = fixture_from_declaration(declaration);
        assert_eq!(
            decode(&fixture.capability).validate_against(&fixture.foundation),
            Ok(fixture.capability)
        );
    }
}

#[test]
fn string_role_rejects_value_and_generic_source_shapes() {
    for kind in [
        SourceNominalKind::Struct,
        SourceNominalKind::Enum,
        SourceNominalKind::Interface,
    ] {
        let declaration = SourceDeclarationKey::nominal(
            SourceDeclarationSite::new(
                scoop_identity::ConeIdentity::CORE,
                PackagePath::root(),
                DefinitionOwnerChain::top_level(),
                DeclarationScope::ConeWide,
            )
            .unwrap(),
            CanonicalIdentifier::new("String").unwrap(),
            kind,
            0,
        );
        let fixture = fixture_from_declaration(declaration);
        assert_eq!(
            decode(&fixture.capability).validate_against(&fixture.foundation),
            Err(
                RuntimeCoreCapabilityValidationError::InvalidStringDeclaration(
                    fixture.capability.source_type()
                )
            )
        );
    }
    let generic = SourceDeclarationKey::nominal(
        SourceDeclarationSite::new(
            scoop_identity::ConeIdentity::CORE,
            PackagePath::root(),
            DefinitionOwnerChain::top_level(),
            DeclarationScope::ConeWide,
        )
        .unwrap(),
        CanonicalIdentifier::new("String").unwrap(),
        SourceNominalKind::Class,
        1,
    );
    assert!(super::validate_string_declaration(&generic).is_err());
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
    fixture_from_declaration(declaration)
}

fn fixture_from_declaration(declaration: SourceDeclarationKey) -> Fixture {
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
