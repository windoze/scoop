use super::*;
use scoop_identity::{
    CanonicalIdentifier, ConeCoordinate, DeclarationScope, DefinitionOwnerChain, ExactTypeKey,
    PackagePath, PendingIdentityValidation, PersistentTypeId, SourceDeclarationKey,
    SourceDeclarationSite, SourceNominalKind, ValidatedIdentityGraph,
};
use scoop_wire::{Encoder, WireEncode, decode_canonical, encode};

#[test]
fn descriptor_wire_retains_core_and_ordinary_providers() {
    for provider in [ConeIdentity::CORE, ordinary_provider()] {
        let descriptor = descriptor(provider, "Visible");
        let bytes = encode(&descriptor).unwrap();
        assert_eq!(bytes[0], 0xa4);
        let decoded: DecodedExternalTypeDescriptor = decode_canonical(&bytes).unwrap();
        let mut identities = identities(provider, descriptor.target());
        assert_eq!(decoded.validate(&mut identities).unwrap(), descriptor);
    }
}

#[test]
fn descriptor_reader_requires_registered_provider_and_target() {
    let provider = ordinary_provider();
    let descriptor = descriptor(provider, "Visible");
    let bytes = encode(&descriptor).unwrap();
    let read = || decode_canonical::<DecodedExternalTypeDescriptor>(&bytes).unwrap();
    let mut pending = PendingIdentityValidation::new();
    pending.register_authority(descriptor.target()).unwrap();
    assert!(matches!(
        read().validate(&mut pending.finish().unwrap()),
        Err(ExternalTypeDescriptorDecodeError::Provider(_))
    ));
    let mut pending = PendingIdentityValidation::new();
    pending.register_authority(provider).unwrap();
    assert!(matches!(
        read().validate(&mut pending.finish().unwrap()),
        Err(ExternalTypeDescriptorDecodeError::Target(_))
    ));
}

#[test]
fn descriptor_reader_rejects_inconsistent_symbol_and_definition() {
    let provider = ordinary_provider();
    let original = descriptor(provider, "Visible");
    let other = descriptor(provider, "Other");
    for changed in [
        ExternalTypeDescriptor::from_selection(
            provider,
            original.target(),
            other.expected_symbol(),
            original.required_definition(),
        ),
        ExternalTypeDescriptor::from_selection(
            provider,
            original.target(),
            original.expected_symbol(),
            other.required_definition(),
        ),
    ] {
        assert_eq!(
            changed.validate_contract(),
            Err(ExternalTypeDescriptorValidationError::ContractMismatch)
        );
        let decoded: DecodedExternalTypeDescriptor =
            decode_canonical(&encode(&changed).unwrap()).unwrap();
        assert!(matches!(
            decoded.validate(&mut identities(provider, original.target())),
            Err(ExternalTypeDescriptorDecodeError::RecordMismatch)
        ));
    }
}

#[test]
fn descriptor_reader_rejects_legacy_implicit_core_product() {
    struct Legacy(ExternalTypeDescriptor);
    impl WireEncode for Legacy {
        fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
            encoder.map(3)?;
            encoder.field(1)?;
            self.0.target().encode(encoder)?;
            encoder.field(2)?;
            self.0.expected_symbol().encode(encoder)?;
            encoder.field(3)?;
            self.0.required_definition().encode(encoder)
        }
    }
    let bytes = encode(&Legacy(descriptor(ConeIdentity::CORE, "String"))).unwrap();
    assert!(decode_canonical::<DecodedExternalTypeDescriptor>(&bytes).is_err());
}

#[test]
fn provider_change_requires_its_own_definition() {
    let original = descriptor(ConeIdentity::CORE, "String");
    let changed = ExternalTypeDescriptor::from_selection(
        ordinary_provider(),
        original.target(),
        original.expected_symbol(),
        original.required_definition(),
    );
    assert_eq!(
        changed.validate_contract(),
        Err(ExternalTypeDescriptorValidationError::ContractMismatch)
    );
}

fn ordinary_provider() -> ConeIdentity {
    ConeCoordinate::new("test", "descriptor-provider", "1.0.0")
        .unwrap()
        .identity()
        .unwrap()
}

fn descriptor(provider: ConeIdentity, name: &str) -> ExternalTypeDescriptor {
    let source = SourceDeclarationKey::nominal(
        SourceDeclarationSite::new(
            provider,
            PackagePath::root(),
            DefinitionOwnerChain::top_level(),
            DeclarationScope::ConeWide,
        )
        .unwrap(),
        CanonicalIdentifier::new(name).unwrap(),
        SourceNominalKind::Class,
        0,
    );
    let target = PersistentExactTypeId::from_key(&ExactTypeKey::Nominal(
        PersistentTypeId::from_source_declaration(&source).unwrap(),
    ))
    .unwrap();
    ExternalTypeDescriptor::new(provider, target).unwrap()
}

fn identities(provider: ConeIdentity, target: PersistentExactTypeId) -> ValidatedIdentityGraph {
    let mut pending = PendingIdentityValidation::new();
    pending.register_authority(provider).unwrap();
    pending.register_authority(target).unwrap();
    pending.finish().unwrap()
}
