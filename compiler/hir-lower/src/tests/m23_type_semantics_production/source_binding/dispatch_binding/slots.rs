use super::*;
use crate::tests::m23_type_semantics_production::source_dispatch::with_hir_source;
use hir::{
    InheritanceCallableDeclarationV1 as Decl, InheritanceSlotContractSemanticAuthority as _,
    InheritanceSlotImplementationV1 as Implementation, InheritanceSlotSourceSemanticAuthority as _,
};
use scoop_identity::{DispatchDeclarationOwner, DispatchRole, PersistentTypeId, PropertyOwner};

mod rejection;
mod support;
use support::*;

#[test]
fn restored_slot_sources_replay_methods_accessors_defaults_and_overrides() {
    for source in [VIRTUAL, INTERFACES, CALLABLES, SELECTIONS] {
        with_hir_source(source, |output, core| {
            let mut fixture = Fixture::from_output(output);
            let sources = Sources::from_output(output, &mut fixture);
            let foundation = fixture.bind().unwrap();
            let dispatch = sources.bind(&foundation, &mut meter()).unwrap();
            let inputs = core
                .foundation
                .import_core_inputs(&core.interface, &[])
                .unwrap();
            let slots = dispatch
                .bind_slot_sources(inputs.protocols().fundamental_types(), &mut meter())
                .unwrap();
            let unit = slots.unit_exact_type().unwrap();
            assert_eq!(
                foundation.exact_type_key(unit).unwrap(),
                &scoop_identity::ExactTypeKey::Nominal(
                    inputs.protocols().fundamental_types().unit().persistent()
                )
            );
            let mut count = 0;
            for selection in sources.selections.records() {
                let record = candidate(&slots, *selection);
                let checked = slots
                    .validate_contract(selection.owner(), &record, &mut meter())
                    .unwrap();
                assert_eq!(checked.owner(), selection.owner());
                assert_eq!(checked.record(), &record);
                count += 1;
            }
            assert!(count > 0);
        });
    }
}

#[test]
fn slot_role_binding_and_replay_share_resource_limits() {
    with_hir_source(INTERFACES, |output, core| {
        let mut fixture = Fixture::from_output(output);
        let sources = Sources::from_output(output, &mut fixture);
        let foundation = fixture.bind().unwrap();
        let dispatch = sources.bind(&foundation, &mut meter()).unwrap();
        let inputs = core
            .foundation
            .import_core_inputs(&core.interface, &[])
            .unwrap();
        for limits in [
            DecodeLimits {
                validation_work_units: 0,
                ..DecodeLimits::default()
            },
            DecodeLimits {
                logical_heap_bytes: 0,
                ..DecodeLimits::default()
            },
            DecodeLimits {
                semantic_table_entries: 0,
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
            DecodeLimits {
                semantic_leaf_bytes: 0,
                ..DecodeLimits::default()
            },
        ] {
            assert!(matches!(
                dispatch.bind_slot_sources(
                    inputs.protocols().fundamental_types(),
                    &mut BudgetMeter::new(limits)
                ),
                Err(hir::InheritanceSlotSourceBindingError::Resource(_))
            ));
        }
        let slots = dispatch
            .bind_slot_sources(inputs.protocols().fundamental_types(), &mut meter())
            .unwrap();
        let selection = sources.selections.records()[0];
        let record = candidate(&slots, selection);
        assert!(
            slots
                .validate_contract(
                    selection.owner(),
                    &record,
                    &mut BudgetMeter::new(DecodeLimits {
                        validation_work_units: 0,
                        ..DecodeLimits::default()
                    })
                )
                .is_err()
        );
    });
}
