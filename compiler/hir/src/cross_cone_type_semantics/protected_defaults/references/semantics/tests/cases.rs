use super::*;

#[test]
fn actual_type_uses_replay_source_target_and_complete_public_domain() {
    let (generic, concrete, metadata) = run(Case::Valid).unwrap();
    assert!(!generic);
    assert_eq!(concrete, 2);
    assert_eq!(metadata, 0);
}
#[test]
fn metadata_occurrences_replay_access_even_with_no_additional_expression_use() {
    let (_, concrete, metadata) = run(Case::Metadata).unwrap();
    assert_eq!((concrete, metadata), (3, 1));
    assert!(matches!(
        run(Case::RejectMetadata),
        Err(ProtectedDefaultReferenceSemanticError::Body(
            ProtectedDefaultBodyClosureError::Source(ProtectedDefaultReferenceAccessError::Source(
                "metadata source access denied"
            ))
        ))
    ));
}
#[test]
fn matching_wire_target_and_origin_do_not_replace_source_authority() {
    assert!(matches!(
        run(Case::WrongTarget),
        Err(ProtectedDefaultReferenceSemanticError::Body(
            ProtectedDefaultBodyClosureError::Source(ProtectedDefaultReferenceAccessError::Source(
                "wrong typed target source"
            ))
        ))
    ));
    assert!(matches!(
        run(Case::Origin),
        Err(ProtectedDefaultReferenceSemanticError::Body(
            ProtectedDefaultBodyClosureError::Source(ProtectedDefaultReferenceAccessError::Origin(
                _
            ))
        ))
    ));
}
#[test]
fn claimed_domain_cannot_replace_actual_target_or_shrink_whole_call_coverage() {
    assert!(matches!(
        run(Case::DomainLie),
        Err(ProtectedDefaultReferenceSemanticError::Body(
            ProtectedDefaultBodyClosureError::Source(
                ProtectedDefaultReferenceAccessError::Coverage(
                    ProtectedDefaultDomainCoverageError::DomainMismatch
                )
            )
        ))
    ));
    assert!(matches!(
        run(Case::DirectCoverage),
        Err(ProtectedDefaultReferenceSemanticError::Body(
            ProtectedDefaultBodyClosureError::Source(
                ProtectedDefaultReferenceAccessError::Coverage(
                    ProtectedDefaultDomainCoverageError::DirectCoverage
                )
            )
        ))
    ));
}
#[test]
fn generic_metadata_replays_all_sources_without_concrete_domain_authority() {
    assert_eq!(run(Case::Generic).unwrap(), (true, 0, 1));
    assert!(matches!(
        run(Case::GenericReject),
        Err(ProtectedDefaultReferenceSemanticError::Body(
            ProtectedDefaultBodyClosureError::Source(ProtectedDefaultReferenceAccessError::Source(
                "metadata source access denied"
            ))
        ))
    ));
}
#[test]
fn empty_and_nonempty_reference_sets_cannot_downgrade_owner_profile() {
    for case in [Case::GenericLie, Case::EmptyReferences] {
        assert!(matches!(
            run(case),
            Err(ProtectedDefaultReferenceSemanticError::OwnerProfile(
                ProtectedDefaultWitnessSourceError::SourceProfile
            ))
        ));
    }
}
