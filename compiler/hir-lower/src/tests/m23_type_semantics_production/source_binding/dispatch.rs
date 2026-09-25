use super::super::source_dispatch::{INTERFACES, VIRTUAL, with_source};
use super::*;

const PROTECTED: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/m23-type-source-dispatch/protected-construction.scoop"
));

#[test]
fn independent_foundation_replays_nonempty_dispatch_and_accessor_sources() {
    for source in [VIRTUAL, INTERFACES] {
        with_source(source, |output, _| {
            let fixture = Fixture::from_output(output);
            let bound = fixture.bind().unwrap();
            let entries = fixture.source.entries();
            let graph = hir::CheckedNominalInheritanceGraphV1::validate_with_source_roots(
                entries.local_inheritance_edges.records().iter(),
                entries.source_roots.values().iter().copied(),
                &bound,
            )
            .unwrap();
            for edge in entries.local_inheritance_edges.records() {
                assert_eq!(graph.get(edge.owner()).unwrap().edges(), edge);
            }
            let callables =
                hir::CanonicalInheritanceSourceCallablesV1::from_dependency_hir(output).unwrap();
            assert!(!callables.records().is_empty());
            for callable in callables.records() {
                assert!(
                    bound.contains_definition_source(
                        callable.declaration_access().definition_origin()
                    )
                );
                if let hir::InheritanceCallableDeclarationV1::Getter(id)
                | hir::InheritanceCallableDeclarationV1::Setter(id) = callable.declaration()
                {
                    assert_eq!(
                        scoop_identity::PersistentPropertyAccessorId::from_key(
                            bound.accessor_key(id).unwrap()
                        )
                        .unwrap(),
                        id
                    );
                }
            }
            let objects = entries
                .representations
                .records()
                .iter()
                .filter(|record| {
                    matches!(
                        record.shape(),
                        hir::NominalRepresentationShapeV1::Object { .. }
                    )
                })
                .collect::<Vec<_>>();
            assert_eq!(objects.len(), 1);
            let exact = PersistentExactTypeId::from_key(&scoop_identity::ExactTypeKey::Nominal(
                objects[0].owner(),
            ))
            .unwrap();
            assert!(graph.object_backing_relation(exact).is_some());
            let production =
                produce_cross_cone_type_semantics(output, &public_interface(output)).unwrap();
            assert!(
                production
                    .section()
                    .inheritance()
                    .records()
                    .iter()
                    .any(|record| !record.slots().records().is_empty())
            );
        });
    }
}

#[test]
fn protected_constructor_does_not_require_candidate_contracts_to_bind_foundation() {
    with_source(PROTECTED, |output, _| {
        let fixture = Fixture::from_output(output);
        let bound = fixture.bind().unwrap();
        let entries = fixture.source.entries();
        assert_eq!(entries.source_roots.values().len(), 3);
        assert_eq!(entries.local_inheritance_edges.records().len(), 3);
        hir::CheckedNominalInheritanceGraphV1::validate_with_source_roots(
            entries.local_inheritance_edges.records().iter(),
            entries.source_roots.values().iter().copied(),
            &bound,
        )
        .unwrap();
        let production =
            produce_cross_cone_type_semantics(output, &public_interface(output)).unwrap();
        assert_eq!(
            production
                .section()
                .protected_declarations()
                .records()
                .len(),
            1
        );
        let sources =
            hir::CanonicalSourceInheritanceInventoriesV1::from_dependency_hir(output).unwrap();
        assert_eq!(
            sources
                .records()
                .iter()
                .map(|record| record.constructors().values().len())
                .sum::<usize>(),
            2
        );
    });
}

#[test]
fn foundation_preserves_generic_supertypes_without_machine_edges() {
    with_source(
        "public interface Generic<T> {}\npublic class User : Generic<Int>",
        |output, _| {
            let foundation =
                hir::CrossConeTypeSemanticsFoundationV1::from_hir(output.output()).unwrap();
            assert!(
                foundation
                    .source_transcript()
                    .unwrap()
                    .entries()
                    .local_inheritance_edges
                    .records()
                    .is_empty()
            );
            assert_eq!(foundation.source_roots().len(), 2);
            let public = public_interface(output);
            assert!(public.nominal_interfaces().records().iter().any(|source| {
                source.exact_supertypes().values().iter().any(|supertype| {
                    matches!(
                        supertype,
                        scoop_identity::SignatureTypeKey::NominalApplication { .. }
                    )
                })
            }));
        },
    );
}

#[test]
fn independent_foundation_bytes_are_deterministic() {
    let first = with_source(INTERFACES, |output, _| {
        let foundation =
            hir::CrossConeTypeSemanticsFoundationV1::from_dependency_hir(output).unwrap();

        encode(&foundation.source_transcript().unwrap()).unwrap()
    });
    let second = with_source(INTERFACES, |output, _| {
        let foundation =
            hir::CrossConeTypeSemanticsFoundationV1::from_dependency_hir(output).unwrap();
        encode(&foundation.source_transcript().unwrap()).unwrap()
    });
    assert_eq!(first, second);
}
