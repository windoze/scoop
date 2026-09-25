use super::*;
use scoop_identity::{CallingConvention, NonEmptyVec};

mod functions;
mod negative;
mod resources;

#[test]
fn shared_extension_receivers_follow_inheritance_for_every_provider_and_accessor_role() {
    let core = Artifact::new(ConeCoordinate::reserved_core()).load(&[]);
    for (provider_coordinate, name) in [
        (ConeCoordinate::reserved_core(), "core"),
        (coordinate("extensions"), "ordinary"),
    ] {
        for form in [CallForm::Function, CallForm::Getter, CallForm::Setter] {
            let mut source = Artifact::new(provider_coordinate.clone());
            let base = source.nominal("Base", SourceNominalKind::Class, &[]);
            let derived = source.nominal("Derived", SourceNominalKind::Class, &[base]);
            let mut provider = source.load(&[]);
            let target = provider.extension(SignatureTypeKey::Nominal(base), "extension", form);
            let dependencies = dependencies(&core, &provider);
            let mut consumer = Artifact::new(coordinate_for_consumer()).load(&dependencies);
            consumer.calls(&provider, &[target, target]);
            consumer.change_last_receiver(exact(derived));
            let actual = consumer.uses(&dependencies).unwrap();
            assert_eq!(
                actual,
                selected(signature_uses(provider.provider(), &[base, derived]))
            );
            consumer
                .validate(&actual, &dependencies, &mut meter())
                .unwrap();
            snapshot(&format!("extension-receiver-{name}"), &actual);
        }
    }
}

#[test]
fn shared_extension_receivers_reach_any_from_nominal_and_structural_values() {
    let core = Artifact::new(ConeCoordinate::reserved_core()).load(&[]);
    let mut source = Artifact::new(coordinate("values"));
    let value = source.nominal("Value", SourceNominalKind::Struct, &[]);
    let mut provider = source.load(&[&core]);
    let any = CoreBuiltinNominal::Any.identity_record().id();
    let target = provider.extension(SignatureTypeKey::Nominal(any), "accept", CallForm::Function);
    let value = SignatureTypeKey::Nominal(value);
    let unit = SignatureTypeKey::Nominal(CoreBuiltinNominal::Unit.identity_record().id());
    for signature in [
        unit.clone(),
        value.clone(),
        SignatureTypeKey::Tuple(NonEmptyVec::new(vec![value.clone(), unit.clone()]).unwrap()),
        SignatureTypeKey::RawPointer(Box::new(value.clone())),
        function(vec![value.clone()], unit.clone()),
        native_function(vec![value], unit),
    ] {
        let actual_type = provider.register_signature(&signature);
        let dependencies = dependencies(&core, &provider);
        let mut consumer = Artifact::new(coordinate_for_consumer()).load(&dependencies);
        consumer.calls(&provider, &[target, target]);
        consumer.change_last_receiver(actual_type);
        let actual = consumer.uses(&dependencies).unwrap();
        assert!(actual.records().iter().all(|record| matches!(
            record.usage(),
            SelectedTypeUseV1::Signature { .. } | SelectedTypeUseV1::Representation { .. }
        )));
        consumer
            .validate(&actual, &dependencies, &mut meter())
            .unwrap();
    }
}

fn provider() -> (Loaded, Loaded, SignatureTypeKey, SignatureTypeKey) {
    let core = Artifact::new(ConeCoordinate::reserved_core()).load(&[]);
    let mut source = Artifact::new(coordinate("extensions"));
    let base = source.nominal("Base", SourceNominalKind::Class, &[]);
    let derived = source.nominal("Derived", SourceNominalKind::Class, &[base]);
    let provider = source.load(&[&core]);
    (
        core,
        provider,
        SignatureTypeKey::Nominal(base),
        SignatureTypeKey::Nominal(derived),
    )
}

fn function(parameters: Vec<SignatureTypeKey>, result: SignatureTypeKey) -> SignatureTypeKey {
    SignatureTypeKey::Function {
        effect: Effect::Ordinary,
        parameters,
        result: Box::new(result),
    }
}

fn native_function(
    parameters: Vec<SignatureTypeKey>,
    result: SignatureTypeKey,
) -> SignatureTypeKey {
    SignatureTypeKey::NativeFunctionPointer {
        calling_convention: CallingConvention::C,
        parameters,
        result: Box::new(result),
    }
}

fn check_relation(
    core: &Loaded,
    provider: &mut Loaded,
    source: &SignatureTypeKey,
    target: &SignatureTypeKey,
    accepted: bool,
) {
    let actual_type = provider.register_signature(source);
    let expected = provider.register_signature(target);
    let extension = provider.extension(target.clone(), "accept", CallForm::Function);
    let dependencies = dependencies(core, provider);
    let mut consumer = Artifact::new(coordinate_for_consumer()).load(&dependencies);
    consumer.calls(provider, &[extension, extension]);
    consumer.change_last_receiver(actual_type);
    if accepted {
        let actual = consumer.uses(&dependencies).unwrap();
        consumer
            .validate(&actual, &dependencies, &mut meter())
            .unwrap();
    } else {
        assert!(
            matches!(consumer.uses(&dependencies), Err(Error::CallReceiver {
            position, receiver: crate::SourceCallReceiver::Receiver { static_type }, expected: actual_expected,
        }) if position.expression_index == 1 && static_type == actual_type && actual_expected == expected)
        );
    }
}
