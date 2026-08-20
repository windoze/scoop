use super::*;

#[test]
fn release_policy_round_trips_with_the_exact_final_owner() {
    let mut fixture = Fixture::new();
    let owner = exact(fixture.class.id()).id();
    let exported = fixture
        .source(
            fixture.class.id(),
            facts(
                MirValueKindV1::Reference,
                MirGcKindV1::ContainsManagedReferences,
            ),
            MirTypeRepresentationV1::Class {
                kind: MirClassKindV1::Final,
                declared_fields: vec![],
                release_policy: MirClassReleasePolicyV1::SynchronousGcFree { owner },
            },
        )
        .unwrap();
    let bytes = encode(&exported).unwrap();
    let decoded: DecodedParamFreeMirTypeExportV1 = decode_canonical(&bytes).unwrap();
    assert_eq!(
        decoded
            .validate(&mut fixture.graph, &fixture.foundation)
            .unwrap(),
        exported
    );
    for (kind, release_owner) in [
        (MirClassKindV1::Final, exact(fixture.other.id()).id()),
        (MirClassKindV1::Open, owner),
        (MirClassKindV1::Abstract, owner),
    ] {
        assert!(matches!(
            fixture.source(
                fixture.class.id(),
                exported.facts(),
                MirTypeRepresentationV1::Class {
                    kind,
                    declared_fields: vec![],
                    release_policy: MirClassReleasePolicyV1::SynchronousGcFree {
                        owner: release_owner
                    },
                },
            ),
            Err(MirTypeBridgeError::ReleaseOwner { exact }) if exact == owner
        ));
    }
}
