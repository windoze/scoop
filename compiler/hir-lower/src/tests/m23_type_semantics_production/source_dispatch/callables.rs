use super::*;
use hir::{
    CanonicalInheritanceSourceCallablesV1 as Table,
    DecodedCanonicalInheritanceSourceCallablesV1 as DecodedTable,
    InheritanceCallableDeclarationV1 as Declaration, InheritanceSourceCallableV1 as Record,
};
use scoop_identity::{CallableTemplateOwner, DefinitionOriginSubject};

mod support;
mod wire;
use support::{render, verify_concrete};

const CALLABLES: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/m23-type-source-dispatch/callables.scoop"
));

#[test]
fn source_callable_contracts_preserve_signature_effects_modality_and_accessor_origins() {
    with_source(CALLABLES, |output, _| {
        let table = table(output);
        verify_concrete(output, &table);
        assert_eq!(
            render(output, &table),
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../tests/fixtures/m23-type-source-dispatch/callables.contracts.snap"
            ))
        );
        roundtrip(output, &table);
    });
}

#[test]
fn source_contracts_cover_exactly_slot_roots_and_selected_targets() {
    for source in [VIRTUAL, INTERFACES, CALLABLES] {
        with_source(source, |output, _| {
            let table = table(output);
            verify_concrete(output, &table);
            let mut expected = std::collections::BTreeSet::new();
            let inventory = project(output);
            let required_slots = inventory
                .records()
                .iter()
                .flat_map(|record| {
                    record
                        .slot_schemas()
                        .records()
                        .iter()
                        .flat_map(|schema| schema.slots().iter().copied())
                })
                .collect::<std::collections::BTreeSet<_>>();
            for record in output
                .output()
                .export
                .module()
                .dispatch_slot_identities
                .records()
            {
                if required_slots.contains(&record.id()) {
                    use scoop_identity::{DispatchDeclarationOwner as Owner, DispatchRole as Role};
                    expected.insert(match (record.key().owner(), record.key().role()) {
                        (Owner::Function(id), _) => Declaration::Function(id),
                        (Owner::Accessor(id), Role::PropertyGetter) => Declaration::Getter(id),
                        (Owner::Accessor(id), Role::PropertySetter) => Declaration::Setter(id),
                        other => panic!("unexpected sealed slot key {other:?}"),
                    });
                }
            }
            let selections = hir::CanonicalInheritanceSourceSlotSelectionsV1::from_ordinary_hir(
                output,
                &mut meter(),
            )
            .unwrap();
            for record in selections.records() {
                use hir::InheritanceSourceSlotSelectionV1 as Selection;
                match record.selection() {
                    Selection::Abstract => {}
                    Selection::Concrete(id) | Selection::InterfaceDefault(id) => {
                        expected.insert(id);
                    }
                }
            }
            assert_eq!(
                table
                    .records()
                    .iter()
                    .map(Record::declaration)
                    .collect::<std::collections::BTreeSet<_>>(),
                expected
            );
            roundtrip(output, &table);
        });
    }
}

#[test]
fn source_contracts_ignore_unrelated_arenas_and_reject_exhausted_projection_budget() {
    let original = with_source(CALLABLES, |output, _| {
        for limits in [
            DecodeLimits {
                validation_work_units: 0,
                ..DecodeLimits::default()
            },
            DecodeLimits {
                logical_heap_bytes: 0,
                ..DecodeLimits::default()
            },
        ] {
            assert!(matches!(
                Table::from_ordinary_hir(output, &mut BudgetMeter::new(limits)),
                Err(hir::CrossConeTypeSemanticsProductionError::SourceInventory(
                    hir::SourceInventoryError::Resource(_)
                ))
            ));
        }
        table(output)
    });
    assert_eq!(
        encode(&original).unwrap(),
        with_source(CALLABLES, |output, _| encode(&table(output)).unwrap())
    );
    let shifted = format!("fun unrelated(): Int = 0\n{CALLABLES}");
    // Access source spans intentionally change when source text moves. Compare
    // signatures and declaration identities, while retaining the new origins.
    with_source(&shifted, |output, _| {
        let shifted = table(output);
        assert_eq!(original.records().len(), shifted.records().len());
        for (a, b) in original.records().iter().zip(shifted.records()) {
            assert_eq!(a.declaration(), b.declaration());
            assert_eq!(a.signature(), b.signature());
            assert_eq!(a.modality(), b.modality());
            assert_eq!(
                a.declaration_access().declared_visibility(),
                b.declaration_access().declared_visibility()
            );
            assert_ne!(
                a.declaration_access().definition_origin(),
                b.declaration_access().definition_origin()
            );
        }
    });
}

fn meter() -> BudgetMeter {
    BudgetMeter::new(DecodeLimits::default())
}
fn table(output: &hir::OrdinaryHirOutput<'_>) -> Table {
    Table::from_ordinary_hir(output, &mut meter()).unwrap()
}
fn roundtrip(output: &hir::OrdinaryHirOutput<'_>, table: &Table) {
    let mut identities = super::super::source_inventory::identity_closure(output);
    let decoded: DecodedTable =
        decode_canonical(&encode(table).unwrap(), DecodeLimits::default()).unwrap();
    assert_eq!(
        decoded.resolve(&mut identities, &mut meter()).unwrap(),
        *table
    );
}

fn template(declaration: Declaration) -> CallableTemplateOwner {
    match declaration {
        Declaration::Function(id) => CallableTemplateOwner::Function(id),
        Declaration::Getter(id) | Declaration::Setter(id) => CallableTemplateOwner::Accessor(id),
    }
}
