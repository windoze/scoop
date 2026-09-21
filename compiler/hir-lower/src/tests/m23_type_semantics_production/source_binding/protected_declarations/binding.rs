use super::*;
use hir::ProtectedDeclarationBindingError as Error;

mod contracts;
mod protocols;

fn with_binding(
    run: impl FnOnce(
        &mut hir::BoundNominalParameterProtocolsV1<'_, '_, '_, '_>,
        &Fixture,
        &hir::CanonicalProtectedDeclarationInterfacesV1,
        &hir::CanonicalProtectedCallableSourceInterfacesV1,
    ),
) {
    with_hir_source(SOURCE, |output, core| {
        let mut fixture = Fixture::from_output(output);
        let sources = Sources::from_output(output, &mut fixture);
        let produced = Production::from_export_hir(&output.output().export, &mut meter()).unwrap();
        let (declarations, protocols) = restore(&mut fixture, &produced);
        let foundation = fixture.bind().unwrap();
        let core = core.foundation.import_core_inputs(&core.interface).unwrap();
        sources.with_bound(
            &foundation,
            core.protocols().fundamental_types(),
            |members, constructors| {
                let mut authority = members
                    .bind_parameter_protocols(constructors, &sources.protocols, &mut meter())
                    .unwrap();
                run(&mut authority, &fixture, &declarations, &protocols);
            },
        );
    });
}

#[test]
fn protected_binding_requires_every_independently_selected_declaration() {
    with_binding(|authority, fixture, table, protocols| {
        let mut counts = [0; 4];
        for record in table.records() {
            counts[match record {
                Declaration::Callable(_) => 0,
                Declaration::Constructor(_) => 1,
                Declaration::Property(_) => 2,
                Declaration::NestedNominal(_) => 3,
            }] += 1;
            let incomplete = hir::CanonicalProtectedDeclarationInterfacesV1::try_new(
                table
                    .records()
                    .iter()
                    .filter(|r| r.reference() != record.reference())
                    .cloned()
                    .collect(),
            )
            .unwrap();
            assert!(matches!(
                authority.validate_protected_declarations(
                    &incomplete,
                    protocols,
                    &fixture.source.entries().representations,
                    &mut meter()
                ),
                Err(Error::Inventory)
            ));
        }
        assert!(counts.into_iter().all(|count| count > 0));
    });
}

#[test]
fn protected_binding_enforces_shared_resource_limits() {
    with_binding(|authority, fixture, table, protocols| {
        for limits in [
            DecodeLimits {
                semantic_table_entries: 0,
                ..DecodeLimits::default()
            },
            DecodeLimits {
                logical_heap_bytes: 0,
                ..DecodeLimits::default()
            },
            DecodeLimits {
                validation_work_units: 0,
                ..DecodeLimits::default()
            },
            DecodeLimits {
                decoded_nodes: 0,
                ..DecodeLimits::default()
            },
            DecodeLimits {
                semantic_recursion: 0,
                ..DecodeLimits::default()
            },
        ] {
            assert!(
                matches!(
                    authority.validate_protected_declarations(
                        table,
                        protocols,
                        &fixture.source.entries().representations,
                        &mut BudgetMeter::new(limits)
                    ),
                    Err(Error::Resource(_))
                ),
                "{limits:?}"
            );
        }
    });
}
