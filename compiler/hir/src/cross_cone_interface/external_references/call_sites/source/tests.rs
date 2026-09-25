use super::*;
use crate::PublicDeclarationOwnerV1;
use scoop_identity::{ExactTypeKey, NonEmptyVec};
use scoop_wire::DecodeLimits;

mod fixture;
mod resources;
use fixture::*;

fn validate(
    fixture: &Fixture,
    site: &HirDependencyCallSiteV1,
) -> Result<(), HirDependencyCallSignatureError> {
    site.validate_source_signature(
        fixture.target,
        fixture.metadata(),
        &mut meter(),
        &WirePath::root(),
    )
}

fn meter() -> BudgetMeter {
    BudgetMeter::new(DecodeLimits::default())
}

#[test]
fn source_call_retains_every_unit_argument() {
    let fixture = Fixture::simple();
    validate(
        &fixture,
        &fixture.call(0, vec![unit_exact(), unit_exact()], unit_exact()),
    )
    .unwrap();
    for arguments in [vec![], vec![unit_exact()], vec![unit_exact(); 3]] {
        assert!(matches!(
            validate(&fixture, &fixture.call(0, arguments, unit_exact())),
            Err(HirDependencyCallSignatureError::ArgumentCount { expected: 2, .. })
        ));
    }
}

#[test]
fn source_call_checks_later_arguments_and_result_at_each_occurrence() {
    let fixture = Fixture::simple();
    validate(
        &fixture,
        &fixture.call(0, vec![unit_exact(); 2], unit_exact()),
    )
    .unwrap();
    assert!(matches!(
        validate(
            &fixture,
            &fixture.call(1, vec![unit_exact(), bool_exact()], unit_exact())
        ),
        Err(HirDependencyCallSignatureError::Argument { index: 1, .. })
    ));
    assert!(matches!(
        validate(
            &fixture,
            &fixture.call(2, vec![unit_exact(); 2], bool_exact())
        ),
        Err(HirDependencyCallSignatureError::Result { .. })
    ));
}

#[test]
fn source_call_joins_extension_receiver_before_declared_parameters() {
    let fixture = Fixture::new(
        PublicDeclarationOwnerV1::Extension,
        Some(boolean()),
        vec![unit()],
        unit(),
        Vec::new(),
    );
    validate(
        &fixture,
        &fixture.call(0, vec![bool_exact(), unit_exact()], unit_exact()),
    )
    .unwrap();
    assert!(matches!(
        validate(
            &fixture,
            &fixture.call(0, vec![unit_exact(), bool_exact()], unit_exact())
        ),
        Err(HirDependencyCallSignatureError::Argument { index: 0, .. })
    ));
}

#[test]
fn source_call_joins_implicit_nominal_receiver() {
    let owner = nominal().id();
    let fixture = Fixture::new(
        PublicDeclarationOwnerV1::Nominal(SourceNominalId::Concrete(owner)),
        None,
        vec![unit()],
        boolean(),
        Vec::new(),
    );
    validate(
        &fixture,
        &fixture.call(
            0,
            vec![exact(ExactTypeKey::Nominal(owner)), unit_exact()],
            bool_exact(),
        ),
    )
    .unwrap();
    assert!(matches!(
        validate(
            &fixture,
            &fixture.call(0, vec![bool_exact(), unit_exact()], bool_exact())
        ),
        Err(HirDependencyCallSignatureError::Argument { index: 0, .. })
    ));
}

#[test]
fn source_call_compares_complete_nested_signature_types() {
    let tuple = ExactTypeKey::Tuple(NonEmptyVec::new(vec![unit_exact(), bool_exact()]).unwrap());
    let reversed = ExactTypeKey::Tuple(NonEmptyVec::new(vec![bool_exact(), unit_exact()]).unwrap());
    let signature = SignatureTypeKey::Tuple(NonEmptyVec::new(vec![unit(), boolean()]).unwrap());
    let fixture = Fixture::new(
        PublicDeclarationOwnerV1::TopLevel,
        None,
        vec![signature.clone()],
        signature,
        vec![tuple.clone(), reversed.clone()],
    );
    validate(
        &fixture,
        &fixture.call(0, vec![exact(tuple.clone())], exact(tuple.clone())),
    )
    .unwrap();
    assert!(matches!(
        validate(
            &fixture,
            &fixture.call(0, vec![exact(reversed.clone())], exact(tuple.clone()))
        ),
        Err(HirDependencyCallSignatureError::Argument { index: 0, .. })
    ));
    assert!(matches!(
        validate(
            &fixture,
            &fixture.call(0, vec![exact(tuple)], exact(reversed))
        ),
        Err(HirDependencyCallSignatureError::Result { .. })
    ));
}

#[test]
fn source_call_requires_concrete_owner_and_signature() {
    let generic = Fixture::new(
        PublicDeclarationOwnerV1::Nominal(SourceNominalId::GenericTemplate(generic_nominal().id())),
        None,
        Vec::new(),
        unit(),
        Vec::new(),
    );
    assert!(matches!(
        validate(&generic, &generic.call(0, vec![], unit_exact())),
        Err(HirDependencyCallSignatureError::GenericDeclaration(_))
    ));
    let unresolved = Fixture::new(
        PublicDeclarationOwnerV1::TopLevel,
        None,
        vec![SignatureTypeKey::Binder { depth: 0, index: 0 }],
        unit(),
        Vec::new(),
    );
    assert!(
        matches!(validate(&unresolved, &unresolved.call(0, vec![unit_exact()], unit_exact())),
        Err(HirDependencyCallSignatureError::Type(error)) if matches!(*error, SharedTypeMetadataError::NonConcreteSignature))
    );
}

#[test]
fn source_call_requires_the_target_declaration_in_its_provider() {
    let mut fixture = Fixture::simple();
    fixture.public = crate::CrossConeHirInterfaceSectionV1::empty();
    assert!(matches!(
        validate(
            &fixture,
            &fixture.call(0, vec![unit_exact(); 2], unit_exact())
        ),
        Err(HirDependencyCallSignatureError::Declaration(_))
    ));
}

#[test]
fn source_signature_cannot_substitute_for_runtime_role_validation() {
    let fixture = Fixture::simple();
    let site = fixture.call(0, Vec::new(), unit_exact());
    let runtime = HirDependencyCallSiteV1::try_new_with_reason(
        site.position(),
        site.origin().clone(),
        Vec::new(),
        unit_exact(),
        HirDependencyCallReasonV1::CastFailure {
            checked_type: bool_exact(),
        },
    )
    .unwrap();
    assert!(matches!(
        validate(&fixture, &runtime),
        Err(HirDependencyCallSignatureError::Reason)
    ));
}
