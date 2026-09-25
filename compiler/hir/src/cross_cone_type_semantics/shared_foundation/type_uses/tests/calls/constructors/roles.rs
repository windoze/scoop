use super::*;

#[test]
fn shared_source_and_runtime_calls_of_one_constructor_keep_both_reasons() {
    let core = Artifact::new(ConeCoordinate::reserved_core()).load(&[]);
    let mut source = Artifact::new(coordinate("provider"));
    let owner = source.nominal("Owner", SourceNominalKind::Class, &[]);
    let mut provider = source.load(&[&core]);
    let target = provider.constructor(owner, Vec::new());
    let dependencies = dependencies(&core, &provider);
    let mut consumer = Artifact::new(coordinate_for_consumer()).load(&dependencies);
    consumer.declaration_calls(&provider, &[target]);
    let reference = consumer
        .metadata()
        .public
        .external_references()
        .records()
        .iter()
        .find(|reference| reference.target() == ExternalHirTargetV1::Callable(target))
        .unwrap()
        .clone();
    let source = reference.call_sites().records()[0].clone();
    let mut position = source.position();
    position.expression_index += 1;
    let runtime = HirDependencyCallSiteV1::try_new_with_reason(
        position,
        source.origin().clone(),
        Vec::new(),
        source.result(),
        HirDependencyCallReasonV1::CastFailure {
            checked_type: exact(owner),
        },
    )
    .unwrap();
    let roles = CanonicalExternalHirReferenceRolesV1::try_new(vec![
        ExternalHirReferenceRoleV1::ConcreteSelectedUse,
        ExternalHirReferenceRoleV1::RuntimeOperationDependency,
    ])
    .unwrap();
    let make = |calls| {
        ExternalHirReferenceV1::try_new(
            reference.origin(),
            reference.target(),
            roles.clone(),
            reference.witnesses().clone(),
            CanonicalHirDependencyCallSitesV1::try_new(calls).unwrap(),
            Default::default(),
        )
    };
    let combined = make(vec![source.clone(), runtime.clone()]).unwrap();
    let decoded: DecodedExternalHirReferenceV1 = scoop_wire::decode_canonical(
        &scoop_wire::encode(&combined).unwrap(),
        DecodeLimits::default(),
    )
    .unwrap();
    assert_eq!(decoded.resolve(&mut consumer.identities).unwrap(), combined);
    for calls in [vec![source], vec![runtime]] {
        assert_eq!(
            make(calls),
            Err(ExternalHirReferenceBuildError::MissingCallSites)
        );
    }
}
