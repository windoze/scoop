use super::*;

#[test]
fn call_domain_replay_preserves_the_nearest_protected_slot_region() {
    let mut builder = Builder::new();
    let base = builder.nominal("Base", Kind::Class, Visibility::Public, 0, vec![], None);
    let original = builder.function(
        base,
        "read",
        Visibility::Protected,
        Modality::Open,
        Dispatch::Virtual,
        &[builder.token.clone()],
    );
    let middle = builder.nominal(
        "Middle",
        Kind::Class,
        Visibility::Public,
        0,
        vec![support::applied(base, vec![])],
        None,
    );
    let inherited = builder.function(
        middle,
        "read",
        Visibility::Protected,
        Modality::Open,
        Dispatch::Inherited(original),
        &[builder.token.clone()],
    );
    let owner = builder.nominal(
        "Derived",
        Kind::Class,
        Visibility::Public,
        0,
        vec![support::applied(middle, vec![])],
        None,
    );
    let function = builder.function(
        owner,
        "read",
        Visibility::Protected,
        Modality::Open,
        Dispatch::Inherited(inherited),
        &[builder.token.clone()],
    );
    let direct = region(vec![Constraint::SubclassesOf(owner)]);
    let slot = region(vec![Constraint::SubclassesOf(base)]);
    let template = builder.template(
        function,
        original,
        vec![],
        Some((direct, Some(slot), Domain::universal())),
    );
    let fixture = builder.finish(vec![template]);
    fixture.validate().unwrap();
    let mut widened_middle = fixture.clone();
    widened_middle.change_callable(inherited, |record| {
        support::callable(
            record,
            Visibility::Public,
            record.modality(),
            record.slot_relations().clone(),
        )
    });
    assert!(matches!(widened_middle.failure(), Error::SlotCoverage(id) if id == inherited));
}

#[test]
fn call_domain_replay_checks_every_generic_inheritance_argument_even_when_unused() {
    let mut builder = Builder::new();
    let base = builder.nominal("Base", Kind::Class, Visibility::Public, 1, vec![], None);
    let original = builder.function(
        base,
        "read",
        Visibility::Public,
        Modality::Open,
        Dispatch::Virtual,
        &[builder.token.clone()],
    );
    let application = SignatureTypeKey::Tuple(scoop_identity::NonEmptyVec::from_first(
        SignatureTypeKey::Binder { depth: 0, index: 0 },
        [builder.token.clone()],
    ));
    let owner = builder.nominal(
        "Derived",
        Kind::Class,
        Visibility::Public,
        1,
        vec![support::applied(base, vec![application.clone()])],
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
    let template = builder.template(function, original, vec![application], None);
    let token = builder.token.clone();
    let fixture = builder.finish(vec![template]);
    fixture.validate().unwrap();
    let mut wrong = fixture.clone();
    wrong.change_mapping(vec![token]);
    assert!(matches!(wrong.failure(), Error::ProviderMapping));
}

#[test]
fn call_domain_replay_matches_applied_parameter_types_without_recovering_identity_from_names() {
    let mut builder = Builder::new();
    let base = builder.nominal("Base", Kind::Class, Visibility::Public, 1, vec![], None);
    let original = builder.function(
        base,
        "read",
        Visibility::Public,
        Modality::Open,
        Dispatch::Virtual,
        &[SignatureTypeKey::Binder { depth: 0, index: 0 }],
    );
    let owner = builder.nominal(
        "Derived",
        Kind::Class,
        Visibility::Public,
        0,
        vec![support::applied(base, vec![builder.token.clone()])],
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
    let template = builder.template(function, original, vec![builder.token.clone()], None);
    builder.clone().finish(vec![template]).validate().unwrap();
    let different = SignatureTypeKey::Tuple(scoop_identity::NonEmptyVec::from_first(
        builder.token.clone(),
        [builder.token.clone()],
    ));
    builder.set_parents(owner, vec![support::applied(base, vec![different])]);
    let template = builder.template(function, original, vec![builder.token.clone()], None);
    assert!(matches!(
        builder.finish(vec![template]).failure(),
        Error::PhysicalSlot
    ));
}

#[test]
fn call_domain_replay_detects_interface_overrides_that_own_distinct_slots() {
    let mut builder = Builder::new();
    let base = builder.nominal("Base", Kind::Interface, Visibility::Public, 0, vec![], None);
    let original = builder.function(
        base,
        "read",
        Visibility::Public,
        Modality::Abstract,
        Dispatch::Interface,
        &[builder.token.clone()],
    );
    let owner = builder.nominal(
        "Derived",
        Kind::Interface,
        Visibility::Public,
        0,
        vec![support::applied(base, vec![])],
        None,
    );
    let function = builder.function(
        owner,
        "read",
        Visibility::Public,
        Modality::InterfaceDefault,
        Dispatch::Interface,
        &[builder.token.clone()],
    );
    let template = builder.template(function, original, vec![], None);
    builder.clone().finish(vec![template]).validate().unwrap();
    let forged = builder.template(function, function, vec![], None);
    assert!(matches!(
        builder.finish(vec![forged]).failure(),
        Error::OriginalProviderOverride
    ));
}

#[test]
fn call_domain_replay_merges_diamond_paths_but_keeps_distinct_interface_applications() {
    let mut builder = Builder::new();
    let base = builder.nominal("Base", Kind::Interface, Visibility::Public, 1, vec![], None);
    let original = builder.function(
        base,
        "read",
        Visibility::Public,
        Modality::Abstract,
        Dispatch::Interface,
        &[builder.token.clone()],
    );
    let application = support::applied(base, vec![builder.token.clone()]);
    let left = builder.nominal(
        "Left",
        Kind::Interface,
        Visibility::Public,
        0,
        vec![application.clone()],
        None,
    );
    let right = builder.nominal(
        "Right",
        Kind::Interface,
        Visibility::Public,
        0,
        vec![application],
        None,
    );
    let owner = builder.nominal(
        "Derived",
        Kind::Struct,
        Visibility::Public,
        0,
        vec![
            support::applied(left, vec![]),
            support::applied(right, vec![]),
        ],
        None,
    );
    let function = builder.function(
        owner,
        "read",
        Visibility::Public,
        Modality::Final,
        Dispatch::Direct,
        &[builder.token.clone()],
    );
    let template = builder.template(function, original, vec![builder.token.clone()], None);
    builder.clone().finish(vec![template]).validate().unwrap();
    let other = SignatureTypeKey::Tuple(scoop_identity::NonEmptyVec::from_first(
        builder.token.clone(),
        [builder.token.clone()],
    ));
    builder.set_parents(right, vec![support::applied(base, vec![other])]);
    let template = builder.template(function, original, vec![builder.token.clone()], None);
    assert!(matches!(
        builder.finish(vec![template]).failure(),
        Error::ProviderMapping
    ));
}
