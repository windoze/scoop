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
        let produced = Production::from_export_hir(&output.output().export).unwrap();
        let (declarations, protocols) = restore(&mut fixture, &produced);
        let foundation = fixture.bind().unwrap();
        let core = core.foundation.import_core_inputs(&core.interface).unwrap();
        sources.with_bound(
            &foundation,
            core.protocols().fundamental_types(),
            |members, constructors| {
                let mut authority = members
                    .bind_parameter_protocols(constructors, &sources.protocols)
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
                    &fixture.source.entries().representations
                ),
                Err(Error::Inventory)
            ));
        }
        assert!(counts.into_iter().all(|count| count > 0));
    });
}
