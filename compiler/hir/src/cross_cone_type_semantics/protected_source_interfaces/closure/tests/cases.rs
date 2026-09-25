use super::*;

#[test]
fn complete_protocol_owners_include_public_and_protected_constructors_once() {
    assert_eq!(validate(Case::Valid).unwrap(), 3);
}
#[test]
fn protocol_inventory_is_not_inferred_from_candidate_rows() {
    for case in [Case::Missing, Case::Extra, Case::NestedMissing] {
        assert!(matches!(
            validate(case),
            Err(ProtectedSourceClosureError::OwnerInventory)
        ));
    }
}
#[test]
fn source_parameter_and_bidirectional_default_closure_are_required() {
    assert!(matches!(
        validate(Case::WrongParameters),
        Err(ProtectedSourceClosureError::Protocol {
            error: ProtectedSourceSemanticError::Arity,
            ..
        })
    ));
    assert!(matches!(
        validate(Case::MissingDefault),
        Err(ProtectedSourceClosureError::DefaultClosure(_))
    ));
}
#[test]
fn recursive_generic_nested_support_keeps_private_source_protocols() {
    assert_eq!(validate(Case::Nested).unwrap(), 5);
}
