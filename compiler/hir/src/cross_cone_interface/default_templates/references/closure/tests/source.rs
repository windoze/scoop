use super::*;
use crate::{SourceAccessConstraintV1, SourceAccessDomainV1};

#[test]
fn shared_reference_closure_preserves_restricted_witnesses() {
    let fixture = Fixture::new();
    let origin = fixture.origin();
    let owner = CallableTemplateOrigin::Function(fixture.function);
    let domain = SourceAccessDomainV1::from_constraints(vec![SourceAccessConstraintV1::Cone(
        ConeIdentity::CORE,
    )])
    .unwrap();
    let witness =
        ExportDefaultAccessWitnessV1::try_new(owner, domain.clone(), None, domain).unwrap();
    let template = template(
        &fixture,
        CanonicalTemplateLocalTableV1::try_new(vec![]).unwrap(),
        body(
            DefaultExpressionKindV1::GlobalRead(fixture.property),
            binder(),
            origin.clone(),
        ),
        reference_set(
            vec![],
            vec![],
            vec![],
            vec![ExportDefaultReferenceV1::new(
                fixture.property,
                origin.clone(),
                witness,
            )],
            vec![],
            vec![],
        ),
        origin,
    );
    template
        .validate_source_reference_closure(&WirePath::root())
        .unwrap();
    assert!(validate(&template, &fixture).is_err());
}

#[test]
fn shared_reference_closure_rejects_multiple_domain_claims_for_one_occurrence() {
    let fixture = Fixture::new();
    let origin = fixture.origin();
    let owner = CallableTemplateOrigin::Function(fixture.function);
    let restricted = ExportDefaultAccessWitnessV1::try_new(
        owner,
        SourceAccessDomainV1::empty(),
        None,
        SourceAccessDomainV1::empty(),
    )
    .unwrap();
    let mut globals = vec![
        reference(fixture.property, &origin, &fixture),
        ExportDefaultReferenceV1::new(fixture.property, origin.clone(), restricted),
    ];
    globals.sort_unstable();
    let template = template(
        &fixture,
        CanonicalTemplateLocalTableV1::try_new(vec![]).unwrap(),
        body(
            DefaultExpressionKindV1::GlobalRead(fixture.property),
            binder(),
            origin.clone(),
        ),
        reference_set(vec![], vec![], vec![], globals, vec![], vec![]),
        origin,
    );
    assert!(matches!(
        template.validate_source_reference_closure(&WirePath::root()),
        Err(ExportDefaultReferenceClosureValidationError::Extra {
            kind: ExportDefaultReferenceKindV1::Global,
            ..
        })
    ));
}
