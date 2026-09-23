use super::*;
use crate::{SourceAccessConstraintV1, SourceAccessDomainV1};

#[test]
fn restricted_source_snapshot_cannot_serve_as_a_public_default_witness() {
    let fixture = Fixture::new();
    let domain = SourceAccessDomainV1::from_constraints(
        vec![SourceAccessConstraintV1::Cone(ConeIdentity::CORE)],
        &mut BudgetMeter::new(DecodeLimits::default()),
        &WirePath::root(),
    )
    .unwrap();
    for restricted_target in [false, true] {
        let (direct, target) = if restricted_target {
            (SourceAccessDomainV1::universal(), domain.clone())
        } else {
            (domain.clone(), SourceAccessDomainV1::universal())
        };
        let witness = ExportDefaultAccessWitnessV1::try_new(
            CallableTemplateOrigin::Function(fixture.function),
            direct,
            None,
            target,
        )
        .unwrap();
        let references = reference_set(
            Vec::new(),
            Vec::new(),
            Vec::new(),
            vec![ExportDefaultReferenceV1::new(
                fixture.property,
                fixture.origin(),
                witness,
            )],
            Vec::new(),
            Vec::new(),
        );
        let template = template(&fixture, references);
        let mut authority = Authority::new(&fixture);
        let error = validate(&template, &fixture, &mut authority).unwrap_err();
        let ExportDefaultReferenceSetSemanticValidationError::Record { error, .. } = error else {
            panic!("reference validation error")
        };
        if restricted_target {
            assert!(matches!(
                *error,
                ExportDefaultReferenceValidationError::RestrictedTargetDomain
            ));
        } else {
            assert!(matches!(
                *error,
                ExportDefaultReferenceValidationError::CallDomain { actual: None, .. }
            ));
        }
        assert_eq!(authority.origins, 0);
        assert!(authority.targets.is_empty());
    }
}
