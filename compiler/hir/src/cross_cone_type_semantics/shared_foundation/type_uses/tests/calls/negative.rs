use super::*;

#[test]
fn shared_call_signatures_reject_missing_extra_and_misdirected_selections() {
    let core = Artifact::new(ConeCoordinate::reserved_core()).load(&[]);
    let mut source = Artifact::new(coordinate("provider"));
    let owner = source.nominal("Owner", SourceNominalKind::Class, &[]);
    let other = source.nominal("Other", SourceNominalKind::Class, &[]);
    let mut provider = source.load(&[&core]);
    let method = provider.callable(nominal_owner(owner), "run", CallForm::Function);
    provider.callable(nominal_owner(other), "unused", CallForm::Function);
    let dependencies = dependencies(&core, &provider);
    let mut consumer = Artifact::new(coordinate_for_consumer()).load(&dependencies);
    consumer.calls(&provider, &[method]);
    let actual = consumer.uses(&dependencies).unwrap();
    let position = actual
        .records()
        .iter()
        .position(|use_| *use_ == signature(provider.provider(), owner))
        .unwrap();
    let mut missing = actual.records().to_vec();
    missing.remove(position);
    assert!(matches!(
        consumer.validate(&selected(missing), &dependencies, &mut meter()),
        Err(Error::TypeUseInventory)
    ));
    for replacement in [
        signature(core.provider(), owner),
        signature(provider.provider(), other),
    ] {
        let mut changed = actual.records().to_vec();
        changed[position] = replacement;
        assert!(matches!(
            consumer.validate(&selected(changed), &dependencies, &mut meter()),
            Err(Error::TypeUseInventory)
        ));
    }
    let mut extra = actual.records().to_vec();
    extra.push(signature(provider.provider(), other));
    assert!(matches!(
        consumer.validate(&selected(extra), &dependencies, &mut meter()),
        Err(Error::TypeUseInventory)
    ));
    assert!(
        matches!(consumer.uses(&[&core]), Err(Error::MissingProvider(origin)) if origin == provider.provider())
    );
}

#[test]
fn shared_call_signatures_check_later_calls_before_deduplicating_type_demand() {
    let core = Artifact::new(ConeCoordinate::reserved_core()).load(&[]);
    let mut source = Artifact::new(coordinate("provider"));
    let owner = source.nominal("Owner", SourceNominalKind::Class, &[]);
    let mut provider = source.load(&[&core]);
    let method = provider.callable(nominal_owner(owner), "run", CallForm::Function);
    let dependencies = dependencies(&core, &provider);
    let mut consumer = Artifact::new(coordinate_for_consumer()).load(&dependencies);
    consumer.calls(&provider, &[method, method]);
    consumer.change_last_argument(exact(CoreBuiltinNominal::Unit.identity_record().id()));
    assert!(
        matches!(consumer.uses(&dependencies), Err(Error::CallSignature { position, source })
        if position.expression_index == 1 && matches!(*source, HirDependencyCallSignatureError::Argument { index: 0, .. }))
    );
}
