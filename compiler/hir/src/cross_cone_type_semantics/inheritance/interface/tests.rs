use super::*;
use crate::cross_cone_type_semantics::inheritance::tests::support::Node;
use crate::cross_cone_type_semantics::protected_interfaces::tests::support::{Fixture, nominal};
use crate::*;
use scoop_identity::{CallableTemplateOrigin, PersistentDispatchSlotId};
use scoop_wire::{BudgetMeter, DecodeLimits, decode_canonical, encode};

mod constructors;
mod generic_constructor;
mod joins;
mod support;
mod wire_tests;
use support::*;

fn meter() -> BudgetMeter {
    BudgetMeter::new(DecodeLimits::default())
}

#[test]
fn complete_inheritance_surface_joins_constructors_protected_members_and_derived_slots() {
    let mut bundle = fixture();
    let bytes = encode(&bundle.table).unwrap();
    let decoded: DecodedCanonicalNominalInheritanceInterfacesV1 =
        decode_canonical(&bytes, DecodeLimits::default()).unwrap();
    assert_eq!(encode(&decoded).unwrap(), bytes);
    assert_eq!(
        decoded.resolve(&mut bundle.fixture, &mut meter()).unwrap(),
        bundle.table
    );
    bundle.validate().unwrap();
    let base = bundle.table.get(bundle.base.exact).unwrap();
    let derived = bundle.table.get(bundle.derived.exact).unwrap();
    assert_eq!(base.slot_schemas(), derived.slot_schemas());
    assert_eq!(
        base.slots().records()[0].slot(),
        derived.slots().records()[0].slot()
    );
    assert_ne!(
        base.slots().records()[0].implementation(),
        derived.slots().records()[0].implementation()
    );
    assert_eq!(
        derived.edges().direct_base(),
        DirectClassBaseV1::ClassBase {
            exact: bundle.base.exact
        }
    );
}

#[test]
fn inheritance_surface_requires_independent_owner_constructor_and_member_inventories() {
    for field in 0..3 {
        let mut bundle = fixture();
        match field {
            0 => bundle.fixture.inheritance_interfaces.owners = CanonicalPersistentIdsV1::empty(),
            1 => {
                bundle
                    .fixture
                    .inheritance_interfaces
                    .constructors
                    .insert(bundle.base.exact, CanonicalPersistentIdsV1::empty());
            }
            2 => {
                bundle.fixture.inheritance_interfaces.members.insert(
                    bundle.base.exact,
                    CanonicalProtectedDeclarationRefsV1::default(),
                );
            }
            _ => unreachable!(),
        }
        assert!(matches!(
            bundle.validate(),
            Err(InheritanceInterfaceSemanticError::Inventory)
        ));
    }
}
