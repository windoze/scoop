use super::*;

#[test]
fn complete_protocol_owners_include_public_and_protected_constructors_once() {
    assert_eq!(validate(Case::Valid, DecodeLimits::default()).unwrap(), 3);
}
#[test]
fn protocol_inventory_is_not_inferred_from_candidate_rows() {
    for case in [Case::Missing, Case::Extra, Case::NestedMissing] {
        assert!(matches!(
            validate(case, DecodeLimits::default()),
            Err(ProtectedSourceClosureError::OwnerInventory)
        ));
    }
}
#[test]
fn source_parameter_and_bidirectional_default_closure_are_required() {
    assert!(matches!(
        validate(Case::WrongParameters, DecodeLimits::default()),
        Err(ProtectedSourceClosureError::Protocol {
            error: ProtectedSourceSemanticError::Arity,
            ..
        })
    ));
    assert!(matches!(
        validate(Case::MissingDefault, DecodeLimits::default()),
        Err(ProtectedSourceClosureError::DefaultClosure(_))
    ));
}
#[test]
fn recursive_generic_nested_support_keeps_private_source_protocols() {
    assert_eq!(validate(Case::Nested, DecodeLimits::default()).unwrap(), 5);
}
#[test]
fn source_inventory_and_protocol_replay_share_resource_limits() {
    for limits in [
        DecodeLimits {
            decoded_nodes: 0,
            ..DecodeLimits::default()
        },
        DecodeLimits {
            logical_heap_bytes: 0,
            ..DecodeLimits::default()
        },
        DecodeLimits {
            validation_work_units: 0,
            ..DecodeLimits::default()
        },
        DecodeLimits {
            semantic_table_entries: 0,
            ..DecodeLimits::default()
        },
    ] {
        assert!(matches!(
            validate(Case::Valid, limits),
            Err(ProtectedSourceClosureError::Resource(_))
        ));
    }
}
