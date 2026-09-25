use super::*;
use crate::{DefaultLocalFunctionV1, DefaultStatementKindV1, DefaultStatementV1};
mod support;
use support::*;

#[test]
fn reference_local_signature_uses_independent_own_arity_and_all_provider_frames() {
    let fixture = Fixture::new();
    let (local, mut authority) = local(&fixture);
    let signature = signature(binder(0, 1), binder(2, 0));
    let candidate = candidate(&fixture, local, signature.clone(), signature, false);
    assert_eq!(validate_scope(&fixture, &candidate, &mut authority), Ok(()));
}

#[test]
fn reference_local_signature_rejects_unknown_identity_and_out_of_scope_binders() {
    let fixture = Fixture::new();
    for signature in [
        signature(binder(0, 2), binder(2, 0)),
        signature(binder(0, 1), binder(3, 0)),
    ] {
        let (local, mut authority) = local(&fixture);
        let candidate = candidate(&fixture, local, signature.clone(), signature, false);
        assert!(
            matches!(validate_scope(&fixture, &candidate, &mut authority),
                Err(ExportDefaultReferenceSetSemanticValidationError::Record { error, .. })
                    if matches!(*error, ExportDefaultReferenceValidationError::Type { .. })
            )
        );
    }
    let (local, mut authority) = local(&fixture);
    let signature = signature(binder(0, 1), binder(2, 0));
    let candidate = candidate(&fixture, local, signature.clone(), signature, false);
    authority.local_keys.clear();
    assert!(
        matches!(validate_scope(&fixture, &candidate, &mut authority),
            Err(ExportDefaultReferenceSetSemanticValidationError::Record { error, .. })
                if matches!(*error, ExportDefaultReferenceValidationError::Target(AuthorityError::MissingLocalFunction))
        )
    );
}

#[test]
fn a_matching_local_signature_cannot_lend_its_scope_to_an_ordinary_occurrence() {
    let fixture = Fixture::new();
    let (local, mut authority) = local(&fixture);
    let signature = signature(binder(0, 1), binder(2, 0));
    let candidate = candidate(&fixture, local, signature.clone(), signature, true);
    assert!(
        matches!(validate_scope(&fixture, &candidate, &mut authority),
            Err(ExportDefaultReferenceSetSemanticValidationError::Record { error, .. })
                if matches!(*error, ExportDefaultReferenceValidationError::Type { .. })
        )
    );
}

#[test]
fn unmatched_type_records_cannot_borrow_a_local_frame_from_a_different_target() {
    let fixture = Fixture::new();
    let (local, mut authority) = local(&fixture);
    let candidate = candidate(
        &fixture,
        local,
        signature(binder(0, 1), binder(2, 0)),
        signature(binder(0, 0), binder(2, 0)),
        false,
    );
    assert!(
        matches!(validate_scope(&fixture, &candidate, &mut authority),
            Err(ExportDefaultReferenceSetSemanticValidationError::Record { error, .. })
                if matches!(*error, ExportDefaultReferenceValidationError::Type { .. })
        )
    );
}
