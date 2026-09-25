use super::*;

#[test]
fn shared_extension_receivers_reject_wrong_identity_and_impostor_any_at_the_actual_call() {
    let core = Artifact::new(ConeCoordinate::reserved_core()).load(&[]);
    let mut source = Artifact::new(coordinate("impostors"));
    let same_name = source.nominal("Base", SourceNominalKind::Class, &[]);
    let impostors = source.load(&[&core]);
    for form in [CallForm::Function, CallForm::Getter, CallForm::Setter] {
        for name in ["Base", "Any"] {
            let mut source = Artifact::new(coordinate("extensions"));
            let base = source.nominal(name, SourceNominalKind::Class, &[]);
            let value = source.nominal("Value", SourceNominalKind::Struct, &[]);
            let mut provider = source.load(&[&core, &impostors]);
            let extension = provider.extension(SignatureTypeKey::Nominal(base), "accept", form);
            let dependencies = vec![&core, &impostors, &provider];
            for receiver in [same_name, value] {
                let mut consumer = Artifact::new(coordinate_for_consumer()).load(&dependencies);
                consumer.calls(&provider, &[extension, extension]);
                let actual = consumer.uses(&dependencies).unwrap();
                consumer.change_last_receiver(exact(receiver));
                let error = consumer.validate(&actual, &dependencies).unwrap_err();
                assert!(matches!(error, Error::CallReceiver {
                    position, receiver: crate::SourceCallReceiver::Receiver { static_type }, expected,
                } if position.expression_index == 1 && static_type == exact(receiver) && expected == exact(base)));
            }
        }
    }
}
