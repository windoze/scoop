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
        with_hir_source(source, |output, _| {
            let mut fixture = Fixture::from_output(output);
            let sources = Sources::from_output(output, &mut fixture);
            let foundation = fixture.bind().unwrap();
            let dispatch = sources.bind(&foundation).unwrap();

            let slots = dispatch.bind_slot_sources().unwrap();
            let unit = slots.unit_exact_type().unwrap();
            assert_eq!(
                foundation.exact_type_key(unit).unwrap(),
                &scoop_identity::ExactTypeKey::Nominal(
                    scoop_identity::CoreBuiltinNominal::Unit
                        .identity_record()
                        .id()
                )
            );
            let mut count = 0;
            for selection in sources.selections.records() {
                let record = candidate(&slots, *selection);
                let checked = slots.validate_contract(selection.owner(), &record).unwrap();
                assert_eq!(checked.owner(), selection.owner());
                assert_eq!(checked.record(), &record);
                count += 1;
            }
            assert!(count > 0);
        });
    }
}
