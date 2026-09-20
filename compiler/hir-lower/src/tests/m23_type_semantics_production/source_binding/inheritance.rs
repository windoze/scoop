use super::super::source_dispatch::with_hir_source as with_source;
use super::*;
use hir::{
    InheritanceSlotSchemaSemanticAuthority as _, InheritanceSlotSourceSemanticAuthority as _,
    InheritanceSourceBindingError as Error, NominalInheritanceInterfaceSemanticAuthority as _,
    NominalSupportCallableSemanticAuthority as _, ProtectedCallableSemanticAuthority as _,
};
use scoop_identity::CallableTemplateOrigin;

mod rejection;
mod support;
mod variants;
use support::*;

const SOURCE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/m23-type-source-nominals/inheritance.scoop"
));
const COMBINED: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/m23-type-source-dispatch/protected-callables.scoop"
));

#[test]
fn restored_inheritance_sources_replay_constructors_and_all_query_roles() {
    for input in [SOURCE, COMBINED] {
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
                        assert_eq!(bound.provider(), fixture.source.entries().provider);
                        let inventory = &sources.properties.dispatch.inventory;
                        assert_eq!(
                            bound.required_inheritance_owners().unwrap(),
                            inventory.owners()
                        );
                        for row in inventory.records() {
                            assert_eq!(
                                bound
                                    .required_inheritance_constructors(row.owner())
                                    .unwrap(),
                                row.constructors()
                            );
                            assert_eq!(
                                bound
                                    .required_inheritance_protected_members(row.owner())
                                    .unwrap(),
                                row.protected_members()
                            );
                            assert_eq!(bound.schemas(row.owner()).unwrap(), row.slot_schemas());
                        }
                        for record in sources.constructors.records() {
                            assert_eq!(
                                bound.constructor_source(record.declaration()).unwrap(),
                                record
                            );
                            let key = bound
                                .callable_source_key(CallableTemplateOrigin::Constructor(
                                    record.declaration(),
                                ))
                                .unwrap();
                            assert_eq!(
                                scoop_identity::PersistentConstructorId::from_source_declaration(
                                    key
                                )
                                .unwrap(),
                                record.declaration()
                            );
                            record.validate_source(graph, bound, &mut meter()).unwrap();
                        }
                        for record in sources.callables.records() {
                            record.validate_source(graph, bound, &mut meter()).unwrap();
                            if let CallableTemplateOrigin::Accessor(id) = record.declaration() {
                                assert_eq!(
                                    bound.property_accessor_key(id).unwrap(),
                                    foundation.accessor_key(id).unwrap()
                                );
                            }
                        }
                        for source in sources.nominals.records() {
                            assert_eq!(
                                bound.source_nominal_modality(source.owner()).unwrap(),
                                source.modality()
                            );
                        }
                        let dispatch = sources
                            .properties
                            .dispatch
                            .bind(&foundation, &mut meter())
                            .unwrap();
                        for row in inventory.records() {
                            for schema in row.slot_schemas().records() {
                                for slot in schema.slots() {
                                    assert_eq!(
                                        bound
                                            .inheritance_slot_selection(row.owner(), *slot)
                                            .unwrap(),
                                        dispatch.selection(row.owner(), *slot).unwrap()
                                    );
                                    assert_eq!(
                                        bound.dispatch_slot_key(*slot).unwrap(),
                                        dispatch.dispatch_slot_key(*slot).unwrap()
                                    );
                                }
                            }
                        }
                    },
                )
                .unwrap();
        });
    }
}
