use super::*;
use crate::cross_cone_type_semantics::inheritance::inheritance_interface_fixture;

#[test]
fn every_inheritance_constructor_slot_root_and_target_keeps_its_own_source_use() {
    let bundle = inheritance_interface_fixture();
    let mut fixture = Fixture::default();
    for record in bundle.table.records() {
        for constructor in record.constructors().records() {
            let source = bundle.fixture.inheritance_interfaces.constructor_sources
                [&constructor.declaration()]
                .declaration_access()
                .definition_origin();
            fixture.expect(
                UseKey::Constructor(record.owner(), constructor.declaration()),
                source,
            );
        }
        for slot in record.slots().records() {
            let (_, _, root) =
                &bundle.fixture.inheritance_interfaces.callables[&slot.declaration()];
            fixture.expect(
                UseKey::Slot(record.owner(), slot.slot()),
                root.definition_origin(),
            );
            let target = slot.implementation().target().unwrap();
            let (_, _, source) =
                &bundle.fixture.inheritance_interfaces.callables[&target.declaration()];
            fixture.expect(
                UseKey::Implementation(record.owner(), slot.slot(), target.declaration()),
                source.definition_origin(),
            );
        }
    }
    assert_eq!(fixture.expected.len(), 6);
    fixture.inheritance = bundle.table;
    let declared = fixture.declared();
    assert!(declared.sources().len() < fixture.expected.len());
    assert_eq!(
        fixture
            .validate(&declared, DecodeLimits::default())
            .unwrap()
            .0,
        6
    );
}
