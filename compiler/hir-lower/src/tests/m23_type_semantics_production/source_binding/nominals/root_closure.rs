use super::*;
use hir::DeclaredVisibilityV1 as Visibility;

const ROOTS: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/m23-type-source-nominals/roots.scoop"
));

#[test]
fn foundation_binds_complete_nested_source_roots_with_real_concrete_support() {
    with_source(ROOTS, |output, _| {
        let mut fixture = Fixture::from_output(output);
        let table = sources(output, &mut fixture);
        let foundation = fixture.bind().unwrap();
        let bound = foundation.bind_nominal_sources(&table).unwrap();
        assert_eq!(bound.table().records().len(), 24);
        let entries = fixture.source.entries();
        assert_eq!(entries.representation_owners.values().len(), 23);
        assert_eq!(entries.local_inheritance_edges.records().len(), 23);
        for (name, visibility, depth) in [
            ("Entry", Visibility::Protected, 1),
            ("Helper", Visibility::Private, 2),
            ("Node", Visibility::Internal, 3),
            ("Last", Visibility::Private, 4),
            ("GenericChild", Visibility::Private, 2),
            ("ConcreteLeaf", Visibility::Private, 3),
            ("PublicChild", Visibility::Public, 2),
            ("InheritedNested", Visibility::Protected, 2),
        ] {
            let record = named(&fixture, &table, name);
            let access = foundation.nominal_source(record.owner()).unwrap().access();
            assert_eq!(access.declared_visibility(), visibility, "{name}");
            assert_eq!(access.lexical_owners().len(), depth, "{name}");
            if let hir::SourceNominalId::Concrete(owner) = record.owner() {
                assert!(
                    entries.representation_owners.values().contains(&owner),
                    "{name}"
                );
                let exact = scoop_identity::PersistentExactTypeId::from_key(
                    &scoop_identity::ExactTypeKey::Nominal(owner),
                )
                .unwrap();
                assert!(
                    output
                        .output()
                        .local
                        .module()
                        .exact_type_identities
                        .type_for_identity(exact)
                        .is_some(),
                    "{name}"
                );
                assert!(
                    entries
                        .local_inheritance_edges
                        .records()
                        .iter()
                        .any(|edge| edge.owner() == exact),
                    "{name}"
                );
            }
        }
        let generic = named(&fixture, &table, "GenericChild");
        let leaf = named(&fixture, &table, "ConcreteLeaf");
        assert!(matches!(
            generic.owner(),
            hir::SourceNominalId::GenericTemplate(_)
        ));
        assert!(matches!(leaf.owner(), hir::SourceNominalId::Concrete(_)));
        assert_eq!(generic.type_parameters().binders().len(), 1);
        assert!(leaf.type_parameters().binders().is_empty());
        assert_eq!(generic.children().values(), &[leaf.owner()]);

        let missing = Table::try_new(
            table
                .records()
                .iter()
                .filter(|r| r.owner() != leaf.owner())
                .cloned()
                .collect(),
        )
        .unwrap();
        assert!(matches!(
            foundation.bind_nominal_sources(&missing),
            Err(Error::Inventory("nominal owners"))
        ));
    });
}
