use super::source_dispatch::with_hir_source as with_source;
use super::*;
use hir::CanonicalInheritanceSourceProtectedCallablesV1 as Table;
use scoop_identity::{CallableTemplateOrigin, DefinitionOriginSubject};
use scoop_wire::{decode_canonical, encode};

mod contracts;
mod shapes;
mod wire;

const SOURCE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/m23-type-source-dispatch/protected-callables.scoop"
));
const DIRECT: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/m23-type-source-dispatch/protected-direct.scoop"
));

fn table(output: &hir::DependencyHirOutput) -> Table {
    Table::from_dependency_hir(output).unwrap()
}

#[test]
fn protected_sources_preserve_methods_accessors_binders_effects_and_slots() {
    with_source(SOURCE, |output, _| {
        let table = table(output);
        contracts::verify(output, &table);
        assert_eq!(
            contracts::render(output, &table),
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../tests/fixtures/m23-type-source-dispatch/protected-callables.contracts.snap"
            ))
        );
    });
}

#[test]
fn nested_protected_source_contracts_have_a_stable_dump() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/m23-type-source-dispatch/protected-binding.scoop"
    ));
    with_source(source, |output, _| {
        let table = table(output);
        contracts::verify(output, &table);
        assert_eq!(
            contracts::render(output, &table),
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../tests/fixtures/m23-type-source-dispatch/protected-binding.contracts.snap"
            ))
        );
    });
}

#[test]
fn protected_source_inventory_excludes_other_visibilities_and_logical_properties() {
    for source in [
        DIRECT,
        SOURCE,
        source_dispatch::VIRTUAL,
        source_dispatch::INTERFACES,
    ] {
        with_source(source, |output, _| {
            let table = table(output);
            let inventory =
                hir::CanonicalSourceInheritanceInventoriesV1::from_dependency_hir(output).unwrap();
            let expected = inventory
                .records()
                .iter()
                .flat_map(|owner| owner.protected_members().values())
                .filter_map(|member| match member {
                    hir::ProtectedDeclarationRefV1::Callable(declaration) => {
                        Some(declaration.declaration())
                    }
                    hir::ProtectedDeclarationRefV1::Constructor(_)
                    | hir::ProtectedDeclarationRefV1::Property(_)
                    | hir::ProtectedDeclarationRefV1::NestedNominal(_) => None,
                })
                .collect::<BTreeSet<_>>();
            assert_eq!(
                table
                    .records()
                    .iter()
                    .map(|record| record.declaration())
                    .collect::<BTreeSet<_>>(),
                expected
            );
            let public = hir::CanonicalCallableInterfacesV1::from_export_hir(
                output.output().export.module(),
            )
            .unwrap();
            for record in table.records() {
                assert!(public.get(record.declaration()).is_none());
                assert_eq!(table.get(record.declaration()), Some(record));
            }
            contracts::verify(output, &table);
        });
    }
}

#[test]
fn protected_source_bytes_replay_without_candidate_or_compiler_local_ids() {
    for source in [DIRECT, SOURCE] {
        let first = with_source(source, |output, _| {
            let table = table(output);
            let bytes = encode(&table).unwrap();
            let decoded: hir::DecodedCanonicalInheritanceSourceProtectedCallablesV1 =
                decode_canonical(&bytes).unwrap();
            assert_eq!(encode(&decoded).unwrap(), bytes);
            let mut identities = source_inventory::identity_closure(output);
            assert_eq!(decoded.resolve(&mut identities).unwrap(), table);
            bytes
        });
        assert_eq!(
            first,
            with_source(source, |output, _| encode(&table(output)).unwrap())
        );
    }
}
