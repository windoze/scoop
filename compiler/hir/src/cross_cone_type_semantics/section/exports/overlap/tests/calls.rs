use super::*;

#[test]
fn overlapping_callable_compares_the_real_owner_signature_and_effects() {
    let mut fixture = SourceFixture::default();
    let owner = fixture.class("PublicOwner");
    let declaration = fixture.function(owner, "member", false, vec![unit()]);
    let payload = fixture.payload(owner, declaration, vec![unit()], unit());
    let public = empty_public(vec![callable(declaration, &payload)]);
    let graph = CheckedNominalInheritanceGraphV1::validate(
        fixture.graph.records.values(),
        &fixture.graph,
        &mut meter(),
    )
    .unwrap();
    let access = fixture.access(owner, DeclaredVisibilityV1::Public);
    callables::validate::<&str>(
        declaration,
        &access,
        &payload,
        &public,
        &graph,
        &mut meter(),
        &path(),
    )
    .unwrap();
    let wrong = fixture.payload(
        owner,
        declaration,
        vec![SignatureTypeKey::RawPointer(Box::new(unit()))],
        unit(),
    );
    assert!(matches!(
        callables::validate::<&str>(
            declaration,
            &access,
            &wrong,
            &public,
            &graph,
            &mut meter(),
            &path()
        ),
        Err(TypeSectionExportValidationError::PublicOverlap)
    ));
    let protected = fixture.access(owner, DeclaredVisibilityV1::Protected);
    assert!(matches!(
        callables::validate::<&str>(
            declaration,
            &protected,
            &payload,
            &public,
            &graph,
            &mut meter(),
            &path()
        ),
        Err(TypeSectionExportValidationError::PublicOverlap)
    ));
}

#[test]
fn declared_public_member_does_not_erase_a_restricted_owner() {
    let mut fixture = SourceFixture::default();
    let owner = fixture.class("HiddenOwner");
    let original = fixture.graph.access[&owner.source].clone();
    fixture.graph.access.insert(
        owner.source,
        DeclarationAccessSourceV1::try_new(
            DeclaredVisibilityV1::Internal,
            vec![],
            original.definition_origin().clone(),
        )
        .unwrap(),
    );
    let declaration = fixture.function(owner, "member", false, vec![]);
    let payload = fixture.payload(owner, declaration, vec![], unit());
    let access = fixture.access(owner, DeclaredVisibilityV1::Public);
    let graph = CheckedNominalInheritanceGraphV1::validate(
        fixture.graph.records.values(),
        &fixture.graph,
        &mut meter(),
    )
    .unwrap();
    assert!(!effective_public::<&str>(&access, &graph, &mut meter(), &path()).unwrap());
    assert!(
        callables::validate::<&str>(
            declaration,
            &access,
            &payload,
            &empty_public(vec![callable(declaration, &payload)]),
            &graph,
            &mut meter(),
            &path()
        )
        .is_err()
    );
    callables::validate::<&str>(
        declaration,
        &access,
        &payload,
        &empty_public(vec![]),
        &graph,
        &mut meter(),
        &path(),
    )
    .unwrap();
}
