use super::super::source_dispatch::with_hir_source;
use super::dispatch_binding::Sources as Dispatch;
use super::nominal_parameters::support::Sources;
use super::*;
use hir::NominalDispatchBindingError as Error;

mod contracts;
mod inventory;
mod resources;
mod selections;

const SOURCE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/m23-type-source-nominals/dispatch-join.scoop"
));

fn with_sources(
    source: &str,
    run: impl FnOnce(&Fixture, &Sources, &Dispatch, &hir::ImportedCoreFundamentalTypeProtocol),
) {
    with_hir_source(source, |output, core| {
        let mut fixture = Fixture::from_output(output);
        let sources = Sources::from_output(output, &mut fixture);
        let dispatch = Dispatch::from_output(output, &mut fixture);
        let core = core
            .foundation
            .import_core_inputs(&core.interface, &[])
            .unwrap();
        run(
            &fixture,
            &sources,
            &dispatch,
            core.protocols().fundamental_types(),
        );
    });
}

#[test]
fn complete_nominal_sources_join_restored_dispatch_contracts_and_inventory() {
    for source in [
        SOURCE,
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/fixtures/m23-type-source-dispatch/selections.scoop"
        )),
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/fixtures/m23-type-source-dispatch/protected-callables.scoop"
        )),
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/fixtures/m23-type-source-dispatch/properties.scoop"
        )),
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/fixtures/m23-type-source-nominals/nested-binding.scoop"
        )),
    ] {
        with_sources(source, |fixture, sources, dispatch, core| {
            let foundation = fixture.bind().unwrap();
            let bound = dispatch.bind(&foundation, &mut meter()).unwrap();
            let slots = bound.bind_slot_sources(core, &mut meter()).unwrap();
            sources.with_bound(&foundation, core, |members, constructors| {
                let parameters = members
                    .bind_parameter_protocols(constructors, &sources.protocols, &mut meter())
                    .unwrap();
                let joined = parameters
                    .bind_dispatch_sources(&slots, &mut meter())
                    .unwrap();
                assert_eq!(joined.provider(), fixture.source.entries().provider);
                assert!(std::ptr::eq(joined.parameters(), &parameters));
                assert!(std::ptr::eq(joined.slots(), &slots));
                assert!(std::ptr::eq(joined.inventory(), &dispatch.inventory));
                if source == SOURCE {
                    let mut rows = Vec::new();
                    for row in joined.inventory().records() {
                        let owner = slots.graph().get(row.owner()).unwrap().source();
                        let scoop_identity::DeclarationName::Named(name) =
                            foundation.nominal_key(owner).unwrap().name()
                        else {
                            panic!("nominal name");
                        };
                        rows.push(format!(
                            "{name}: constructors={}, protected={}\n",
                            row.constructors().values().len(),
                            row.protected_members().values().len()
                        ));
                    }
                    rows.sort();
                    assert_eq!(
                        rows.concat(),
                        include_str!(concat!(
                            env!("CARGO_MANIFEST_DIR"),
                            "/../../tests/fixtures/m23-type-source-nominals/dispatch-join.snap"
                        ))
                    );
                }
            });
        });
    }
}
