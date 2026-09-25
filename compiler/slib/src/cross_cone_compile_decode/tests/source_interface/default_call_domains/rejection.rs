use super::*;

#[test]
fn call_domain_replay_rejects_a_slot_borrowed_from_an_unrelated_same_signature_method() {
    let mut builder = Builder::new();
    let first = builder.nominal("First", Kind::Class, Visibility::Public, 0, vec![], None);
    let first = builder.function(
        first,
        "read",
        Visibility::Public,
        Modality::Open,
        Dispatch::Virtual,
        &[builder.token.clone()],
    );
    let second = builder.nominal("Second", Kind::Class, Visibility::Public, 0, vec![], None);
    let second = builder.function(
        second,
        "read",
        Visibility::Public,
        Modality::Open,
        Dispatch::Virtual,
        &[builder.token.clone()],
    );
    assert_ne!(first, second);
    let template = builder.template(first, first, vec![], None);
    let mut fixture = builder.finish(vec![template]);
    fixture.validate().unwrap();
    let slots = fixture
        .interface
        .callable_interfaces()
        .declaration(second)
        .unwrap()
        .slot_relations()
        .clone();
    fixture.change_callable(first, |record| {
        support::callable(
            record,
            record.declared_visibility(),
            record.modality(),
            slots,
        )
    });
    assert!(matches!(fixture.failure(), Error::PhysicalSlot));
}

#[test]
fn call_domain_replay_rejects_default_providers_outside_the_actual_ancestor_graph() {
    let mut builder = Builder::new();
    let first = builder.nominal("First", Kind::Class, Visibility::Public, 0, vec![], None);
    let first = builder.function(
        first,
        "read",
        Visibility::Public,
        Modality::Final,
        Dispatch::Direct,
        &[builder.token.clone()],
    );
    let second = builder.nominal("Second", Kind::Class, Visibility::Public, 0, vec![], None);
    let second = builder.function(
        second,
        "read",
        Visibility::Public,
        Modality::Final,
        Dispatch::Direct,
        &[builder.token.clone()],
    );
    let template = builder.template(first, second, vec![], None);
    assert!(matches!(
        builder.finish(vec![template]).failure(),
        Error::InheritedProvider
    ));
}

#[test]
fn call_domain_replay_rejects_final_overrides_and_effect_mismatches() {
    let mut builder = Builder::new();
    let base = builder.nominal("Base", Kind::Class, Visibility::Public, 0, vec![], None);
    let original = builder.function(
        base,
        "read",
        Visibility::Public,
        Modality::Open,
        Dispatch::Virtual,
        &[builder.token.clone()],
    );
    let owner = builder.nominal(
        "Derived",
        Kind::Class,
        Visibility::Public,
        0,
        vec![support::applied(base, vec![])],
        None,
    );
    let function = builder.function(
        owner,
        "read",
        Visibility::Public,
        Modality::Open,
        Dispatch::Inherited(original),
        &[builder.token.clone()],
    );
    let template = builder.template(function, original, vec![], None);
    let fixture = builder.finish(vec![template]);
    fixture.validate().unwrap();
    let mut final_base = fixture.clone();
    final_base.change_callable(original, |record| {
        support::callable(
            record,
            record.declared_visibility(),
            Modality::Final,
            record.slot_relations().clone(),
        )
    });
    assert!(matches!(final_base.failure(), Error::FinalOverride(id) if id == original));
    let mut effects = fixture.clone();
    effects.change_callable(original, |record| {
        let effect = record.effects();
        scoop_hir::CallableDeclarationRecordV1::try_new(
            record.declaration(),
            record.owner(),
            record.type_parameters().clone(),
            record.receiver().cloned(),
            record.parameters().clone(),
            record.result().clone(),
            scoop_hir::CallableSourceEffectsV1::try_new(
                scoop_identity::Effect::Suspend,
                effect.safety(),
                effect.gc_effect(),
                effect.implementation(),
                effect.operator_role(),
                effect.infix(),
            )
            .unwrap(),
            record.modality(),
            record.declared_visibility(),
            record.slot_relations().clone(),
        )
        .unwrap()
    });
    assert!(matches!(effects.failure(), Error::SignatureEffects(id) if id == original));
}

#[test]
fn call_domain_replay_rejects_cyclic_inheritance() {
    let mut builder = Builder::new();
    let owner = builder.nominal("Cycle", Kind::Class, Visibility::Public, 0, vec![], None);
    let function = builder.function(
        owner,
        "read",
        Visibility::Public,
        Modality::Open,
        Dispatch::Virtual,
        &[builder.token.clone()],
    );
    let template = builder.template(function, function, vec![], None);
    let fixture = builder.clone().finish(vec![template]);
    fixture.validate().unwrap();

    builder.set_parents(owner, vec![support::applied(owner, vec![])]);
    let template = builder.template(function, function, vec![], None);
    assert!(
        matches!(builder.finish(vec![template]).failure(), Error::NominalCycle(id) if id == owner)
    );
}
