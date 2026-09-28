use super::*;
use scoop_identity::{DeclarationName, ExactTypeKey, PersistentExactTypeId, SourceDeclarationKey};

#[test]
fn actual_core_type_surface_keeps_generic_inheritance_source_only() {
    let sources = sources::core_sources_with(&[]);
    let world =
        scoop_hir::ImportedSemanticWorld::from_dependencies(sources.cone(), Vec::new(), Vec::new())
            .unwrap();
    let hir = crate::request::preflight::current_hir::CurrentConeHirArtifacts::lower(
        scoop_identity::RequestedConeKind::Library,
        &sources,
        scoop_hir_lower::CoreProtocolInput::CurrentDeclarations,
        &world,
    )
    .unwrap();
    let input = hir.machine_input();

    let source_foundation =
        scoop_hir::CanonicalHirFoundation::from_type_semantics_output(input.output).unwrap();
    let decoded: scoop_hir::DecodedHirFoundation =
        scoop_wire::decode_canonical(&scoop_wire::encode(&source_foundation).unwrap()).unwrap();
    let mut pending = scoop_identity::PendingIdentityValidation::new();
    pending
        .register_authority(input.output.output().export.cone)
        .unwrap();
    decoded.register_identities(&mut pending).unwrap();
    decoded.resolve_identities(&mut pending).unwrap();
    let identities = pending.finish().unwrap();
    let production = scoop_hir::CrossConeTypeSemanticsSectionV1::from_dependency_hir(
        input.output,
        scoop_hir::SharedTypeMetadataV1 {
            provider: input.output.output().export.cone,
            identities: &identities,
            foundation: &source_foundation,
            public: input.public,
        },
        &[],
    )
    .unwrap();
    let mut found = std::collections::BTreeSet::new();
    for declaration in input.public.nominal_interfaces().all_records() {
        let key = match declaration.declaration() {
            scoop_hir::SourceNominalId::Concrete(id) => identities
                .canonical_key::<_, SourceDeclarationKey>(id)
                .unwrap(),
            scoop_hir::SourceNominalId::GenericTemplate(id) => identities
                .canonical_key::<_, SourceDeclarationKey>(id)
                .unwrap(),
        };
        let DeclarationName::Named(name) = key.name() else {
            continue;
        };
        if !matches!(name.as_str(), "IntRange" | "Int" | "String" | "Throwable") {
            continue;
        }
        let scoop_hir::SourceNominalId::Concrete(owner) = declaration.declaration() else {
            panic!("expected a non-generic declaration");
        };
        let exact = PersistentExactTypeId::from_key(&ExactTypeKey::Nominal(owner)).unwrap();
        let machine = name.as_str() != "IntRange";
        assert_eq!(
            production.representation_support().get(owner).is_some(),
            machine
        );
        assert_eq!(production.inheritance().get(exact).is_some(), machine);
        if !machine {
            assert!(
                declaration
                    .exact_supertypes()
                    .values()
                    .iter()
                    .any(|supertype| {
                        matches!(supertype, SignatureTypeKey::NominalApplication { .. })
                    })
            );
            assert!(!declaration.members().members().is_empty());
            assert!(
                !declaration
                    .declaration_details()
                    .constructors()
                    .values()
                    .is_empty()
            );
        }
        found.insert(name.as_str().to_owned());
    }
    assert_eq!(
        found,
        ["Int", "IntRange", "String", "Throwable"]
            .into_iter()
            .map(str::to_owned)
            .collect()
    );
}
