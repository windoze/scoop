use super::*;
use hir::{
    ProtectedDeclarationInterfaceV1 as Declaration, TypeSectionDeclarationSemanticAuthority,
};
use scoop_identity::CallableTemplateOrigin;

mod rejection;

const ENTRY: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/m23-type-source-nominals/declaration-entry.scoop"
));
fn with_entry(
    source: &str,
    run: impl FnOnce(
        &mut hir::BoundNominalDispatchSourcesV1<'_, '_, '_, '_, '_>,
        &hir::CanonicalProtectedDeclarationInterfacesV1,
        &hir::CanonicalProtectedCallableSourceInterfacesV1,
        &hir::CanonicalNominalRepresentationSupportV1,
        &hir::CheckedNominalInheritanceGraphV1<'_>,
    ),
) {
    with_hir_source(source, |output, core| {
        let mut fixture = Fixture::from_output(output);
        let sources = Sources::from_output(output, &mut fixture);
        let dispatch = Dispatch::from_output(output, &mut fixture);
        let produced = hir::ProtectedDeclarationSourceProductionV1::from_export_hir(
            &output.output().export,
            &mut meter(),
        )
        .unwrap();
        let (table, protocols) =
            super::super::protected_declarations::support::restore(&mut fixture, &produced);
        let foundation = fixture.bind().unwrap();
        let core = core
            .foundation
            .import_core_inputs(&core.interface, &[])
            .unwrap();
        let core = core.protocols().fundamental_types();
        let dispatch = dispatch.bind(&foundation, &mut meter()).unwrap();
        let slots = dispatch.bind_slot_sources(core, &mut meter()).unwrap();
        sources.with_bound(&foundation, core, |members, constructors| {
            let parameters = members
                .bind_parameter_protocols(constructors, &sources.protocols, &mut meter())
                .unwrap();
            let mut joined = parameters
                .bind_dispatch_sources(&slots, &mut meter())
                .unwrap();
            run(
                &mut joined,
                &table,
                &protocols,
                &fixture.source.entries().representations,
                slots.graph(),
            );
        });
    });
}

#[test]
fn section_declaration_entry_replays_complete_restored_source_contracts() {
    for source in [
        ENTRY,
        SOURCE,
        "public class Plain {}",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/fixtures/m23-type-source-nominals/nested-binding.scoop"
        )),
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/fixtures/m23-type-source-nominals/protected-declarations.scoop"
        )),
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/fixtures/m23-type-source-dispatch/properties.scoop"
        )),
    ] {
        with_entry(
            source,
            |authority, table, protocols, representations, graph| {
                let checked = authority
                    .validate_protected_sources(
                        table,
                        protocols,
                        representations,
                        graph,
                        &mut meter(),
                    )
                    .unwrap();
                assert!(std::ptr::eq(checked.table(), table));
                if source == ENTRY {
                    let mut counts = [0; 4];
                    for record in checked.table().records() {
                        counts[match record {
                            Declaration::Callable(_) => 0,
                            Declaration::Constructor(_) => 1,
                            Declaration::Property(_) => 2,
                            Declaration::NestedNominal(_) => 3,
                        }] += 1;
                    }
                    assert_eq!(
                        format!(
                            "callables={}, constructors={}, properties={}, nested={}\n",
                            counts[0], counts[1], counts[2], counts[3]
                        ),
                        include_str!(concat!(
                            env!("CARGO_MANIFEST_DIR"),
                            "/../../tests/fixtures/m23-type-source-nominals/declaration-entry.snap"
                        ))
                    );
                }
            },
        );
    }
}
