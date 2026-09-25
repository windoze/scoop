use super::*;
use scoop_identity::GeneratedNominalKey;

#[test]
fn shared_member_calls_keep_source_objects_distinct_from_their_backing_classes() {
    let core = Artifact::new(ConeCoordinate::reserved_core()).load(&[]);
    let mut source = Artifact::new(coordinate("objects"));
    let base = source.nominal("Base", SourceNominalKind::Class, &[]);
    let object = source.nominal("Registry", SourceNominalKind::Object, &[base]);
    let mut provider = source.load(&[&core]);
    let member = provider.callable(nominal_owner(base), "run", CallForm::Function);
    let backing = CborIdentityRecord::<PersistentTypeId, _>::from_key(
        GeneratedNominalKey::ObjectBackingClass { object },
    )
    .unwrap();
    let mut pending = PendingIdentityValidation::new();
    pending
        .register_external_graph_authorities(&provider.identities)
        .unwrap();
    pending
        .register_external_canonical_authority(backing.clone())
        .unwrap();
    pending
        .register_external_canonical_authority(
            CborIdentityRecord::<PersistentExactTypeId, _>::from_key(ExactTypeKey::Nominal(
                backing.id(),
            ))
            .unwrap(),
        )
        .unwrap();
    provider.identities = pending.finish().unwrap();
    let dependencies = dependencies(&core, &provider);
    let mut consumer = Artifact::new(coordinate_for_consumer()).load(&dependencies);
    consumer.calls(&provider, &[member, member]);
    consumer.change_last_receiver(exact(object));
    let mut expected = signature_uses(provider.provider(), &[base, object]);
    expected.extend([
        member_call(provider.provider(), base, member),
        member_call(provider.provider(), object, member),
    ]);
    let actual = consumer.uses(&dependencies).unwrap();
    assert_eq!(actual, selected(expected));
    consumer
        .validate(&actual, &dependencies, &mut meter())
        .unwrap();
    snapshot("member-source-object", &actual);
    consumer.change_last_receiver(exact(backing.id()));
    assert!(matches!(
        consumer.uses(&dependencies),
        Err(Error::Identity(_))
    ));
}
