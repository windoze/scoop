use scoop_identity::{ConeIdentity, SignatureTypeKey};

use super::*;

mod support;
use support::{nominal, ordinary_provider, section};

#[test]
fn indexes_declared_field_order_for_core_and_ordinary_providers() {
    let core = nominal(ConeIdentity::CORE, 0);
    let ordinary = nominal(ordinary_provider(), 0);
    let core_section = section(vec![core.record.clone()]);
    let ordinary_section = section(vec![ordinary.record.clone()]);
    let index =
        DefaultStructFields::from_interfaces([&ordinary_section, &core_section], &WirePath::root())
            .unwrap();
    for nominal in [&core, &ordinary] {
        for (position, field) in nominal.fields.iter().enumerate() {
            assert_eq!(
                index.field_index(*field, &nominal.applied_type(0)),
                Ok(position as u32)
            );
        }
    }
    assert!(matches!(
        index.field_index(core.fields[0], &ordinary.applied_type(0)),
        Err(CrossConeHirDefaultFieldError::Owner { .. })
    ));
}

#[test]
fn generic_binding_preserves_owner_identity_and_complete_arity() {
    let generic = nominal(ordinary_provider(), 2);
    let provider = section(vec![generic.record.clone()]);
    let index = DefaultStructFields::from_interfaces([&provider], &WirePath::root()).unwrap();
    assert_eq!(
        index.field_index(generic.fields[1], &generic.applied_type(2)),
        Ok(1)
    );
    assert!(matches!(
        index.field_index(generic.fields[1], &generic.applied_type(1)),
        Err(CrossConeHirDefaultFieldError::Arity {
            expected: 2,
            actual: 1,
            ..
        })
    ));
    assert!(matches!(
        index.field_index(
            generic.fields[0],
            &SignatureTypeKey::Binder { depth: 0, index: 0 }
        ),
        Err(CrossConeHirDefaultFieldError::NonNominalOwner(_))
    ));
}

#[test]
fn a_field_is_unavailable_until_its_provider_is_in_the_explicit_closure() {
    let external = nominal(ordinary_provider(), 0);
    let current = section(vec![]);
    let index = DefaultStructFields::from_interfaces([&current], &WirePath::root()).unwrap();
    assert_eq!(
        index.field_index(external.fields[0], &external.applied_type(0)),
        Err(CrossConeHirDefaultFieldError::MissingField(
            external.fields[0]
        ))
    );
    let provider = section(vec![external.record.clone()]);
    assert!(matches!(
        DefaultStructFields::from_interfaces([&provider, &provider], &WirePath::root(),),
        Err(CrossConeHirDefaultDataFlowError::DuplicateField(_))
    ));
}
