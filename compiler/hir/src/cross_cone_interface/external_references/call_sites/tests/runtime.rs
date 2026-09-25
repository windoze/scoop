use super::*;

fn cast(fixture: &Fixture) -> HirDependencyCallSiteV1 {
    let source = fixture.site(4, vec![0]).unwrap();
    HirDependencyCallSiteV1::try_new_with_reason(
        source.position(),
        source.origin().clone(),
        Vec::new(),
        fixture.unit,
        HirDependencyCallReasonV1::CastFailure {
            checked_type: fixture.unit,
        },
    )
    .unwrap()
}

#[test]
fn runtime_call_reason_round_trips_without_a_source_name_witness() {
    let fixture = Fixture::new();
    let site = cast(&fixture);
    assert!(site.witness_indices().is_empty());
    let decoded: DecodedHirDependencyCallSiteV1 =
        decode_canonical(&encode(&site).unwrap(), DecodeLimits::default()).unwrap();
    assert_eq!(
        decoded
            .resolve(&mut fixture.graph(), &mut meter(), &WirePath::root())
            .unwrap(),
        site
    );
    assert_eq!(
        HirDependencyCallSiteV1::try_new_with_reason(
            site.position(),
            site.origin().clone(),
            vec![fixture.unit],
            fixture.unit,
            site.reason().clone(),
        ),
        Err(HirDependencyCallSiteBuildError::RuntimeArguments)
    );
}

#[test]
fn call_reasons_reject_legacy_arrays_and_unknown_tags() {
    let fixture = Fixture::new();
    let site = fixture.site(0, vec![0]).unwrap();
    let reason = encode(site.reason()).unwrap();
    for suffix in [vec![0x81, 0], vec![0xa2, 0, 3, 1, 0], vec![0xa1, 0, 1]] {
        let mut bytes = encode(&site).unwrap();
        bytes.truncate(bytes.len() - reason.len());
        bytes.extend(suffix);
        assert!(
            decode_canonical::<DecodedHirDependencyCallSiteV1>(&bytes, DecodeLimits::default())
                .is_err()
        );
    }
}

#[test]
fn runtime_reason_and_reference_role_cannot_impersonate_an_ordinary_call() {
    let fixture = Fixture::new();
    let site = cast(&fixture);
    let make = |role, witnesses| {
        ExternalHirReferenceV1::try_new(
            fixture.provider,
            fixture.target(),
            CanonicalExternalHirReferenceRolesV1::try_new(vec![role]).unwrap(),
            witnesses,
            CanonicalHirDependencyCallSitesV1::try_new(vec![site.clone()]).unwrap(),
            Default::default(),
        )
    };
    assert_eq!(
        make(
            ExternalHirReferenceRoleV1::RuntimeOperationDependency,
            crate::CanonicalDependencyBindingWitnessesV1::try_new(Vec::new()).unwrap()
        ),
        Err(ExternalHirReferenceBuildError::RuntimeTarget)
    );
    assert_eq!(
        make(
            ExternalHirReferenceRoleV1::ConcreteSelectedUse,
            fixture.witnesses()
        ),
        Err(ExternalHirReferenceBuildError::CallReason { site: 0 })
    );
    assert_eq!(
        make(
            ExternalHirReferenceRoleV1::RuntimeOperationDependency,
            fixture.witnesses()
        ),
        Err(ExternalHirReferenceBuildError::UnexpectedWitness)
    );
}
