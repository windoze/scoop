use super::*;
use scoop_hir::NominalInheritanceSemanticAuthority;
use scoop_identity::{DeclarationName, ExactTypeKey, PersistentExactTypeId};
use scoop_wire::WirePath;

#[test]
fn actual_core_type_surface_keeps_generic_inheritance_source_only() {
    let sources = sources::core_sources_with(&[]);
    let hir = super::super::super::TrustedCoreBootstrapHirOutput::lower(&sources).unwrap();
    let input = hir.machine_input();

    let source_foundation = scoop_hir::OdrFreeHirFoundation::try_new(
        scoop_hir::CanonicalHirFoundation::from_type_semantics_output(input.output).unwrap(),
    )
    .unwrap();
    let decoded: scoop_hir::DecodedHirFoundation = scoop_wire::decode_canonical(
        &scoop_wire::encode(source_foundation.as_canonical()).unwrap(),
    )
    .unwrap();
    let mut pending = scoop_identity::PendingIdentityValidation::new();
    pending
        .register_authority(input.output.output().export.cone)
        .unwrap();
    decoded.register_identities(&mut pending).unwrap();
    decoded.resolve_identities(&mut pending).unwrap();
    let identities = pending.finish().unwrap();
    let production = scoop_hir::CrossConeTypeSemanticsProductionV1::from_dependency_hir(
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
    let foundation = production.foundation();
    let graph = scoop_hir::CheckedNominalInheritanceGraphV1::validate_with_source_roots(
        production.local_inheritance_edges().iter(),
        production.source_roots().iter().copied(),
        foundation,
    )
    .unwrap();
    production
        .section()
        .representation_support()
        .validate_source_semantics(foundation, &WirePath::root())
        .unwrap();
    let mut found = std::collections::BTreeSet::new();
    for declaration in production.source_nominals().records() {
        let key = foundation
            .nominal_declaration_key(declaration.owner())
            .unwrap();
        let DeclarationName::Named(name) = key.name() else {
            continue;
        };
        if !matches!(name.as_str(), "IntRange" | "Int" | "String" | "Throwable") {
            continue;
        }
        let scoop_hir::SourceNominalId::Concrete(owner) = declaration.owner() else {
            panic!("expected a non-generic declaration");
        };
        let exact = PersistentExactTypeId::from_key(&ExactTypeKey::Nominal(owner)).unwrap();
        let machine = name.as_str() != "IntRange";
        assert_eq!(
            production
                .section()
                .representation_support()
                .get(owner)
                .is_some(),
            machine
        );
        assert_eq!(
            production.section().inheritance().get(exact).is_some(),
            machine
        );
        assert_eq!(graph.get(exact).is_some(), machine);
        let public = input
            .public
            .nominal_interfaces()
            .get(declaration.owner())
            .unwrap();
        assert_eq!(public.source_shape(), declaration.source_shape());
        assert_eq!(public.exact_supertypes(), declaration.supertypes());
        if !machine {
            assert!(declaration.supertypes().values().iter().any(|supertype| {
                matches!(supertype, SignatureTypeKey::NominalApplication { .. })
            }));
            assert!(!public.members().members().is_empty());
            assert!(!declaration.constructors().values().is_empty());
        }
        found.insert(name.as_str());
    }
    assert_eq!(
        found,
        ["Int", "IntRange", "String", "Throwable"]
            .into_iter()
            .collect()
    );
}
