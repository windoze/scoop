use super::*;
use hir::{
    InheritanceCallableDeclarationV1 as Callable, InheritanceSourceSlotSelectionV1 as Selection,
};
use scoop_identity::CallableTemplateOwner;

mod concrete;
mod render;

const SELECTIONS: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/m23-type-source-dispatch/selections.scoop"
));

fn selections(output: &hir::OrdinaryHirOutput) -> hir::CanonicalInheritanceSourceSlotSelectionsV1 {
    hir::CanonicalInheritanceSourceSlotSelectionsV1::from_ordinary_hir(
        output,
        &mut BudgetMeter::new(DecodeLimits::default()),
    )
    .unwrap()
}

#[test]
fn source_selections_preserve_abstract_overrides_class_winners_and_value_methods() {
    with_source(SELECTIONS, |output, mir| {
        let table = selections(output);
        concrete::verify(output, mir, &table);
        let rendered = render::render(output, &table);
        assert_eq!(
            rendered,
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../tests/fixtures/m23-type-source-dispatch/selections.choices.snap"
            ))
        );
        roundtrip(output, &table);
    });
}

#[test]
fn source_selections_match_diamond_defaults_getters_setters_and_virtual_families() {
    for source in [INTERFACES, VIRTUAL] {
        with_source(source, |output, mir| {
            let table = selections(output);
            concrete::verify(output, mir, &table);
            let source_slots = project(output)
                .records()
                .iter()
                .flat_map(|record| {
                    record.slot_schemas().records().iter().flat_map(|schema| {
                        schema.slots().iter().map(|slot| (record.owner(), *slot))
                    })
                })
                .collect::<std::collections::BTreeSet<_>>();
            assert_eq!(
                table
                    .records()
                    .iter()
                    .map(|record| (record.owner(), record.slot()))
                    .collect::<std::collections::BTreeSet<_>>(),
                source_slots
            );
            let has_role = |role: fn(Callable) -> bool| {
                table
                    .records()
                    .iter()
                    .any(|record| match record.selection() {
                        Selection::Concrete(callable) | Selection::InterfaceDefault(callable) => {
                            role(callable)
                        }
                        Selection::Abstract => false,
                    })
            };
            assert!(has_role(|callable| matches!(callable, Callable::Getter(_))));
            assert!(has_role(|callable| matches!(callable, Callable::Setter(_))));
            roundtrip(output, &table);
        });
    }
}

#[test]
fn source_selection_bytes_ignore_arena_order_and_obey_shared_limits() {
    let first = with_source(SELECTIONS, |output, _| {
        for limits in [
            DecodeLimits {
                semantic_recursion: 2,
                ..DecodeLimits::default()
            },
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
                hir::CanonicalInheritanceSourceSlotSelectionsV1::from_ordinary_hir(
                    output,
                    &mut BudgetMeter::new(limits)
                ),
                Err(hir::CrossConeTypeSemanticsProductionError::SourceInventory(
                    hir::SourceInventoryError::Resource(_)
                ))
            ));
        }
        encode(&selections(output)).unwrap()
    });
    let shifted = format!("fun unrelated(): Int = 17\n{SELECTIONS}");
    let second = with_source(&shifted, |output, _| encode(&selections(output)).unwrap());
    assert_eq!(first, second);
}

#[test]
fn source_selections_reject_generic_dispatch_application_without_odr_authority() {
    with_source(
        "public interface Generic<T> {}\npublic class User : Generic<Int>",
        |output, _| {
            assert!(matches!(
                hir::CanonicalInheritanceSourceSlotSelectionsV1::from_ordinary_hir(
                    output,
                    &mut BudgetMeter::new(DecodeLimits::default())
                ),
                Err(hir::CrossConeTypeSemanticsProductionError::GenericOdrRequired(_))
            ));
        },
    );
}

fn roundtrip(
    output: &hir::OrdinaryHirOutput,
    table: &hir::CanonicalInheritanceSourceSlotSelectionsV1,
) {
    let mut identities = super::super::source_inventory::identity_closure(output);
    let decoded: hir::DecodedCanonicalInheritanceSourceSlotSelectionsV1 =
        decode_canonical(&encode(table).unwrap(), DecodeLimits::default()).unwrap();
    assert_eq!(
        decoded
            .resolve(
                &mut identities,
                &mut BudgetMeter::new(DecodeLimits::default())
            )
            .unwrap(),
        *table
    );
}

fn template(callable: Callable) -> CallableTemplateOwner {
    match callable {
        Callable::Function(id) => CallableTemplateOwner::Function(id),
        Callable::Getter(id) | Callable::Setter(id) => CallableTemplateOwner::Accessor(id),
    }
}
