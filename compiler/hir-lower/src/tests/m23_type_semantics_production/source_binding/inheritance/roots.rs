use super::*;
use scoop_identity::{DeclarationName, ExactTypeKey, PersistentExactTypeId};

const ROOTS: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/m23-type-source-nominals/roots.scoop"
));
const NESTED: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/m23-type-source-nominals/nested-inheritance.scoop"
));

#[test]
fn nested_inheritance_closure_replays_all_bound_sources_after_byte_restoration() {
    for input in [ROOTS, NESTED] {
        with_source(input, |output, core| {
            let mut fixture = Fixture::from_output(output);
            let sources = Sources::from_output(output, &mut fixture);
            let foundation = fixture.bind().unwrap();
            let inputs = core
                .foundation
                .import_core_inputs(&core.interface, &[])
                .unwrap();
            sources
                .with_bound(
                    &foundation,
                    inputs.protocols().fundamental_types(),
                    &mut meter(),
                    |bound, graph| {
                        let entries = fixture.source.entries();
                        assert_eq!(
                            bound.required_inheritance_owners().unwrap().values(),
                            entries
                                .local_inheritance_edges
                                .records()
                                .iter()
                                .map(|edge| edge.owner())
                                .collect::<Vec<_>>()
                        );
                        for record in sources.constructors.records() {
                            record.validate_source(graph, bound, &mut meter()).unwrap();
                        }
                        for record in sources.callables.records() {
                            record.validate_source(graph, bound, &mut meter()).unwrap();
                        }
                        for nominal in sources.nominals.records() {
                            if let hir::SourceNominalId::Concrete(owner) = nominal.owner() {
                                let exact =
                                    PersistentExactTypeId::from_key(&ExactTypeKey::Nominal(owner))
                                        .unwrap();
                                assert!(
                                    output
                                        .output()
                                        .local
                                        .module()
                                        .exact_type_identities
                                        .type_for_identity(exact)
                                        .is_some()
                                );
                                assert!(
                                    bound
                                        .required_inheritance_owners()
                                        .unwrap()
                                        .values()
                                        .contains(&exact)
                                );
                            }
                        }
                    },
                )
                .unwrap();
            if input == NESTED {
                assert_eq!(
                    render(&foundation, &sources),
                    include_str!(concat!(
                        env!("CARGO_MANIFEST_DIR"),
                        "/../../tests/fixtures/m23-type-source-nominals/nested-inheritance.snap"
                    ))
                );
            }
        });
    }
}

fn render(foundation: &hir::BoundTypeFoundationSourcesV1<'_>, sources: &Sources) -> String {
    let mut lines = Vec::new();
    for nominal in sources.nominals.records() {
        let DeclarationName::Named(name) = foundation.nominal_key(nominal.owner()).unwrap().name()
        else {
            panic!("nominal name");
        };
        let access = foundation.nominal_source(nominal.owner()).unwrap().access();
        let detail = match nominal.owner() {
            hir::SourceNominalId::Concrete(owner) => {
                let exact = PersistentExactTypeId::from_key(&ExactTypeKey::Nominal(owner)).unwrap();
                let row = sources.properties.dispatch.inventory.get(exact).unwrap();
                let slots = row
                    .slot_schemas()
                    .records()
                    .iter()
                    .map(|schema| schema.slots().len())
                    .sum::<usize>();
                format!(
                    "constructors={} protected={} slots={slots}",
                    row.constructors().values().len(),
                    row.protected_members().values().len()
                )
            }
            hir::SourceNominalId::GenericTemplate(_) => format!(
                "generic binders={}",
                nominal.type_parameters().binders().len()
            ),
        };
        lines.push(format!(
            "{} {:?} {detail}\n",
            name.as_str(),
            access.declared_visibility()
        ));
    }
    lines.sort();
    lines.concat()
}

#[test]
fn nested_param_free_support_cannot_downgrade_a_generic_base_to_source_only() {
    with_source(
        "public interface Generic<T> {}\npublic open class Host { protected class Nested : Generic<Int> {} }",
        |output, _| {
            assert!(matches!(
                hir::CrossConeTypeSemanticsFoundationV1::from_ordinary_hir(output, &mut meter()),
                Err(hir::CrossConeTypeSemanticsProductionError::GenericOdrRequired(_))
            ));
            assert!(matches!(
                hir::CanonicalSourceInheritanceInventoriesV1::from_ordinary_hir(
                    output,
                    &mut meter()
                ),
                Err(hir::CrossConeTypeSemanticsProductionError::GenericOdrRequired(_))
            ));
            assert!(matches!(
                hir::CanonicalInheritanceSourceSlotSelectionsV1::from_ordinary_hir(
                    output,
                    &mut meter()
                ),
                Err(hir::CrossConeTypeSemanticsProductionError::GenericOdrRequired(_))
            ));
        },
    );
}
