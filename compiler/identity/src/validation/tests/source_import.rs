use super::*;

#[test]
fn source_records_share_existing_world_ids_without_artifact_fingerprints() {
    let mut pending = PendingIdentityValidation::new();
    pending
        .register_canonical(IdentityLayer::Hir, source_type_record())
        .unwrap();
    let graph = pending.finish().unwrap();
    let mut session = SemanticIdentitySession::new();
    let fingerprint = SemanticOriginFingerprint::new([1; 32], [2; 32], [3; 32]);
    let (artifact, _, _) = session
        .import(ConeIdentity::CORE, fingerprint, &graph)
        .unwrap()
        .into_parts();
    let (source, _, _) = session
        .import_source(ConeIdentity::SINGLE_FILE, &graph)
        .unwrap()
        .into_parts();
    assert_eq!(
        artifact.get(source_type_record().id()),
        source.get(source_type_record().id())
    );
    assert_eq!(
        session.origin_fingerprint(ConeIdentity::CORE),
        Some(fingerprint)
    );
    assert_eq!(session.origin_fingerprint(ConeIdentity::SINGLE_FILE), None);
    assert_eq!(session.origin_count(), 2);
    assert_eq!(session.entity_count(), 1);
}

#[test]
fn a_second_source_or_artifact_input_for_the_same_cone_is_not_merged() {
    let mut pending = PendingIdentityValidation::new();
    pending
        .register_canonical(IdentityLayer::Hir, source_type_record())
        .unwrap();
    let graph = pending.finish().unwrap();
    let mut session = SemanticIdentitySession::new();
    session.import_source(ConeIdentity::CORE, &graph).unwrap();
    assert!(matches!(
        session.import_source(ConeIdentity::CORE, &graph),
        Err(SemanticIdentityImportError::OriginConflict { .. })
    ));
    assert!(matches!(
        session.import(
            ConeIdentity::CORE,
            SemanticOriginFingerprint::new([1; 32], [2; 32], [3; 32]),
            &graph
        ),
        Err(SemanticIdentityImportError::OriginConflict { .. })
    ));
    assert_eq!(session.origin_count(), 1);
    assert_eq!(session.entity_count(), 1);
}
