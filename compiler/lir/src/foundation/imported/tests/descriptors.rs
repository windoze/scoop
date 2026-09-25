use super::*;
use crate::{ExternalTypeDescriptor, foundation::CanonicalLirFoundation};
use scoop_identity::{ConeCoordinate, PersistentTypeId, SourceNominalKind};

#[test]
fn descriptor_projection_retains_the_actual_provider_and_selected_definition() {
    for provider in [
        ConeIdentity::CORE,
        ConeCoordinate::new("test", "descriptors", "1.0.0")
            .unwrap()
            .identity()
            .unwrap(),
    ] {
        let exact = exact_type(provider, "Exported");
        let (foundation, definitions) = fixture(provider, exact);
        let selected = foundation
            .project_type_descriptor(&definitions, exact)
            .unwrap();
        let descriptor = selected;
        assert_eq!(descriptor.provider(), provider);
        assert_eq!(descriptor.target(), exact);
        assert_eq!(descriptor.expected_symbol(), selected.expected_symbol());
        assert_eq!(
            descriptor.required_definition(),
            selected.required_definition()
        );
        assert_eq!(
            descriptor,
            ExternalTypeDescriptor::new(provider, exact).unwrap()
        );
        descriptor.validate_definition(&definitions).unwrap();

        let missing = exact_type(provider, "Missing");
        assert!(matches!(
            foundation.project_type_descriptor(&definitions, missing),
            Err(ImportedLirTypeDescriptorProjectionError::MissingExactType(found))
                if found == missing
        ));

        let other = ConeIdentity::SINGLE_FILE;
        let (_, foreign_definitions) = fixture(other, exact);
        assert_eq!(
            descriptor.validate_definition(&foreign_definitions),
            Err(
                crate::ExternalTypeDescriptorValidationError::MissingDefinition(
                    descriptor.required_definition()
                )
            )
        );
        assert!(matches!(
            foundation.project_type_descriptor(&foreign_definitions, exact),
            Err(ImportedLirTypeDescriptorProjectionError::Definition(
                crate::ExternalTypeDescriptorValidationError::MissingDefinition(found)
            ))
                if found == descriptor.required_definition()
        ));
        let foreign = ExternalTypeDescriptor::new(other, exact).unwrap();
        assert_eq!(foreign.expected_symbol(), descriptor.expected_symbol());
        assert_ne!(
            foreign.required_definition(),
            descriptor.required_definition()
        );
    }
}

fn fixture(
    provider: ConeIdentity,
    exact: PersistentExactTypeId,
) -> (ImportedLirFoundation, StrongObjectSymbolSurfaceV1) {
    let descriptor = ExternalTypeDescriptor::new(provider, exact).unwrap();
    let definition = CborIdentityRecord::from_key(
        ObjectDefinitionPlanKey::strong(
            provider,
            StrongDefinitionEntity::exact_type(exact),
            StrongDefinitionRole::TypeDescriptor,
        )
        .unwrap(),
    )
    .unwrap();
    let mut canonical = CanonicalLirFoundation::empty();
    canonical.set_materialized_exact_types(vec![exact]).unwrap();
    canonical
        .set_definition_plans(vec![definition.clone()])
        .unwrap();
    canonical
        .set_definition_atoms(vec![
            CborIdentityRecord::from_key(ObjectDefinitionAtomKey::new(
                definition.id(),
                DefinitionAtomRole::Primary,
                DefinitionAtomSubkey::Singleton,
            ))
            .unwrap(),
        ])
        .unwrap();
    canonical.set_symbol_requests(
        PersistentSymbolRequestTable::new(vec![descriptor.expected_symbol()]).unwrap(),
    );
    let strong = OdrFreeLirFoundation::try_new(provider, canonical.clone()).unwrap();
    let definitions = StrongObjectSymbolSurfaceV1::from_odr_free_foundation(&strong).unwrap();
    let decoded: super::super::super::DecodedLirFoundation =
        decode_canonical(&encode(&canonical).unwrap()).unwrap();
    let mut pending = PendingIdentityValidation::new();
    pending.register_authority(provider).unwrap();
    pending.register_authority(exact).unwrap();
    decoded.register_identities(&mut pending).unwrap();
    decoded.resolve_identities(&mut pending).unwrap();
    let identities = pending.finish().unwrap();
    let mut session = SemanticIdentitySession::new();
    let (_, _, imported) = session
        .import(
            provider,
            SemanticOriginFingerprint::new([1; 32], [2; 32], [3; 32]),
            &identities,
        )
        .unwrap()
        .into_parts();
    (
        ImportedLirFoundation {
            canonical,
            identities: imported,
        },
        definitions,
    )
}

fn exact_type(provider: ConeIdentity, name: &str) -> PersistentExactTypeId {
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
    PersistentExactTypeId::from_key(&ExactTypeKey::Nominal(
        PersistentTypeId::from_source_declaration(&source).unwrap(),
    ))
    .unwrap()
}
