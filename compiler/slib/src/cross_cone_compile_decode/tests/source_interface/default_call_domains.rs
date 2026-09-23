use super::*;
use crate::CrossConeHirDefaultCallDomainError as Error;
use scoop_hir::{
    CallableModalityV1 as Modality, DeclaredVisibilityV1 as Visibility,
    SourceAccessConstraintV1 as Constraint, SourceAccessDomainV1 as Domain,
};
use scoop_wire::DecodeLimits;

mod inheritance;
mod rejection;
mod support;
use support::{Builder, Dispatch, Kind, region};

#[test]
fn call_domain_replay_checks_empty_reference_templates_against_actual_declarations() {
    let mut builder = Builder::new();
    let owner = builder.nominal(
        "Container",
        Kind::Class,
        Visibility::Public,
        0,
        vec![],
        None,
    );
    let function = builder.function(
        owner,
        "read",
        Visibility::Public,
        Modality::Open,
        Dispatch::Virtual,
        &[builder.token.clone()],
    );
    builder.public(function, PublicLookupAccessV1::PublicSlot);
    let template = builder.template(function, function, vec![], None);
    let fixture = builder.finish(vec![template]);
    fixture.validate().unwrap();
    let mut wrong = fixture.clone();
    wrong.change_callable(function, |record| {
        support::callable(
            record,
            record.declared_visibility(),
            Modality::Final,
            CanonicalPersistentIdsV1::empty(),
        )
    });
    assert!(matches!(wrong.failure(), Error::PublicLookup));
}

#[test]
fn call_domain_replay_intersects_all_lexical_owners_for_direct_and_root_slot_access() {
    let mut builder = Builder::new();
    let outer = builder.nominal("Outer", Kind::Class, Visibility::Internal, 0, vec![], None);
    let owner = builder.nominal(
        "Inner",
        Kind::Class,
        Visibility::Private,
        0,
        vec![],
        Some(outer),
    );
    let function = builder.function(
        owner,
        "read",
        Visibility::Public,
        Modality::Open,
        Dispatch::Virtual,
        &[builder.token.clone()],
    );
    let expected = region(vec![
        Constraint::Cone(builder.cone.identity()),
        Constraint::LexicalOwner(outer),
    ]);
    let template = builder.template(
        function,
        function,
        vec![],
        Some((
            expected.clone(),
            Some(expected.clone()),
            Domain::universal(),
        )),
    );
    let fixture = builder.finish(vec![template]);
    fixture.validate().unwrap();
    let mut wrong = fixture.clone();
    wrong.change_witness(Domain::universal(), Some(expected), Domain::universal());
    assert!(matches!(
        wrong.failure(),
        Error::Witness {
            reason: "direct call domain differs from the actual source declaration",
            ..
        }
    ));
}

#[test]
fn call_domain_replay_distinguishes_direct_lookup_from_a_wider_override_slot() {
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
        "Hidden",
        Kind::Class,
        Visibility::Internal,
        0,
        vec![support::applied(base, vec![])],
        None,
    );
    let function = builder.function(
        owner,
        "read",
        Visibility::Public,
        Modality::Final,
        Dispatch::Inherited(original),
        &[builder.token.clone()],
    );
    let direct = region(vec![Constraint::Cone(builder.cone.identity())]);
    let template = builder.template(
        function,
        original,
        vec![],
        Some((
            direct.clone(),
            Some(Domain::universal()),
            Domain::universal(),
        )),
    );
    let fixture = builder.finish(vec![template]);
    fixture.validate().unwrap();
    let mut wrong = fixture.clone();
    wrong.change_witness(direct.clone(), Some(direct.clone()), Domain::universal());
    assert!(matches!(
        wrong.failure(),
        Error::Witness {
            reason: "slot call domain differs from the actual inherited contract",
            ..
        }
    ));
    let mut wrong = fixture.clone();
    wrong.change_witness(direct.clone(), Some(Domain::universal()), direct);
    assert!(matches!(
        wrong.failure(),
        Error::Witness {
            reason: "target access does not cover the complete call domain",
            ..
        }
    ));
}

#[test]
fn call_domain_replay_retains_interface_obligations_without_an_own_physical_slot() {
    for kind in [Kind::Class, Kind::Struct] {
        let mut builder = Builder::new();
        let interface = builder.nominal(
            "Reader",
            Kind::Interface,
            Visibility::Public,
            0,
            vec![],
            None,
        );
        let original = builder.function(
            interface,
            "read",
            Visibility::Public,
            Modality::Abstract,
            Dispatch::Interface,
            &[builder.token.clone()],
        );
        let owner = builder.nominal(
            "Hidden",
            kind,
            Visibility::Internal,
            0,
            vec![support::applied(interface, vec![])],
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
        let direct = region(vec![Constraint::Cone(builder.cone.identity())]);
        let template = builder.template(
            function,
            original,
            vec![],
            Some((
                direct.clone(),
                Some(Domain::universal()),
                Domain::universal(),
            )),
        );
        let fixture = builder.finish(vec![template]);
        fixture.validate().unwrap();
        let mut wrong = fixture.clone();
        wrong.change_witness(direct, None, Domain::universal());
        assert!(matches!(wrong.failure(), Error::Witness { .. }));
    }
}
