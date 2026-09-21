use super::*;
use scoop_identity::PersistentDispatchSlotId;
use scoop_wire::{decode_canonical, encode};

mod callables;
mod replay;
mod selections;
mod support;
use support::*;
pub(super) use support::{with_hir_source, with_hir_source_at, with_hir_sources, with_source};

pub(super) const VIRTUAL: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/m23-type-source-dispatch/virtual.scoop"
));
pub(super) const INTERFACES: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/m23-type-source-dispatch/interfaces.scoop"
));

#[test]
fn source_vtable_keeps_base_prefix_final_override_and_protected_accessors() {
    with_source(VIRTUAL, |output, mir| {
        let inventory = project(output);
        let base = inventory.get(owner(output, "Base")).unwrap();
        let derived = inventory.get(owner(output, "Derived")).unwrap();
        let singleton = inventory.get(owner(output, "Singleton")).unwrap();
        let role = hir::InheritanceSlotSchemaRoleV1::ClassVtable;
        let base_slots = base.slot_schemas().get(role).unwrap().slots();
        let derived_slots = derived.slot_schemas().get(role).unwrap().slots();
        let derived_mir = mir
            .classes
            .iter()
            .find(|(_, class)| class.name == "Derived")
            .unwrap()
            .1;
        assert_eq!(derived_mir.vtable.len(), derived_slots.len());
        assert_eq!(
            derived_mir
                .vtable
                .iter()
                .map(|slot| match slot {
                    scoop_mir::TableSlot::Function(function) =>
                        mir.functions[*function].name.as_str(),
                    scoop_mir::TableSlot::Runtime(_) =>
                        panic!("source virtual slots select user functions"),
                })
                .collect::<Vec<_>>(),
            [
                "Derived.zeta",
                "Base.alpha",
                "Base.$get$value",
                "Base.$set$value",
                "Derived.middle"
            ]
        );
        assert_eq!(base_slots.len(), 4);
        assert_eq!(base.protected_members().values().len(), 3);
        assert_eq!(derived_slots.len(), 5);
        assert!(derived_slots.starts_with(base_slots));
        assert_eq!(
            singleton.slot_schemas().get(role).unwrap().slots(),
            derived_slots
        );
        assert_eq!(
            render(output, &inventory),
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../tests/fixtures/m23-type-source-dispatch/virtual.scoop.snap"
            ))
        );
        roundtrip(output, &inventory);
    });
}

#[test]
fn source_interface_order_matches_concrete_diamond_override_and_value_tables() {
    with_source(INTERFACES, |output, _| {
        let inventory = project(output);
        let local = output.output().local.module();
        for (interface, declaration) in local.interfaces.iter() {
            let exact = local.exact_type_identities[declaration.canonical_type].id();
            let source = inventory.get(exact).unwrap();
            assert_eq!(source.slot_schemas().records().len(), 1);
            let role = hir::InheritanceSlotSchemaRoleV1::Interface {
                interface_exact: exact,
            };
            let actual = local
                .dispatch_slot_identities
                .interface_slots(interface)
                .map(|(_, record)| record.id())
                .collect::<Vec<_>>();
            assert_eq!(source.slot_schemas().get(role).unwrap().slots(), actual);
            for name in ["User", "Derived", "Singleton", "Value", "Choice"] {
                let implementor = inventory.get(owner(output, name)).unwrap();
                if declaration.name == "AbstractAgain"
                    || (declaration.name == "Mutable" && matches!(name, "Value" | "Choice"))
                {
                    assert!(implementor.slot_schemas().get(role).is_none());
                } else {
                    assert_eq!(
                        implementor.slot_schemas().get(role).unwrap().slots(),
                        actual
                    );
                }
            }
        }
        assert_eq!(
            render(output, &inventory),
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../tests/fixtures/m23-type-source-dispatch/interfaces.scoop.snap"
            ))
        );
        roundtrip(output, &inventory);
    });
}

#[test]
fn source_dispatch_bytes_ignore_unrelated_arena_allocation() {
    let first = with_source(INTERFACES, |output, _| encode(&project(output)).unwrap());
    let shifted = format!("fun unrelated(): Int = 7\n{INTERFACES}");
    let second = with_source(&shifted, |output, _| encode(&project(output)).unwrap());
    assert_eq!(first, second);
}

#[test]
fn source_dispatch_graph_traversal_obeys_shared_depth_and_work_limits() {
    with_source(INTERFACES, |output, _| {
        for limits in [
            DecodeLimits {
                semantic_recursion: 3,
                ..DecodeLimits::default()
            },
            DecodeLimits {
                validation_work_units: 0,
                ..DecodeLimits::default()
            },
        ] {
            assert!(matches!(
                hir::CanonicalSourceInheritanceInventoriesV1::from_dependency_hir(
                    output,
                    &mut BudgetMeter::new(limits)
                ),
                Err(hir::CrossConeTypeSemanticsProductionError::SourceInventory(
                    hir::SourceInventoryError::Resource(_)
                ))
            ));
        }
    });
}

fn roundtrip(
    output: &hir::DependencyHirOutput,
    inventory: &hir::CanonicalSourceInheritanceInventoriesV1,
) {
    let mut identities = super::source_inventory::identity_closure(output);
    let restored: hir::DecodedCanonicalSourceInheritanceInventoriesV1 =
        decode_canonical(&encode(inventory).unwrap(), DecodeLimits::default()).unwrap();
    let restored = restored
        .resolve(
            &mut identities,
            &mut BudgetMeter::new(DecodeLimits::default()),
        )
        .unwrap();
    assert_eq!(&restored, inventory);
}

#[test]
fn source_dispatch_rejects_generic_interface_materialization() {
    with_source(
        "public interface Generic<T> {}\npublic class User : Generic<Int>",
        |output, _| {
            assert!(matches!(
                hir::CanonicalSourceInheritanceInventoriesV1::from_dependency_hir(
                    output,
                    &mut BudgetMeter::new(DecodeLimits::default())
                ),
                Err(hir::CrossConeTypeSemanticsProductionError::GenericOdrRequired(_))
            ));
        },
    );
}
