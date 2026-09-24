use scoop_identity::CoreBuiltinNominal;

use super::*;
use crate::{
    CanonicalDependencyBindingWitnessesV1, CanonicalExternalHirReferenceRolesV1,
    ExternalHirReferenceSemanticValidationError, ExternalHirReferenceV1,
};

#[test]
fn builtin_default_dependencies_preserve_actual_provider_and_dependency_uses() {
    for builtin in [CoreBuiltinNominal::Unit, CoreBuiltinNominal::Any] {
        let target = ExternalHirTargetV1::Nominal(NominalDeclarationOwner::Concrete(
            builtin.identity_record().id(),
        ));
        let provider = builtin.declaration_key().origin();
        let mut authority = Authority::new(cone("consumer")).with_origin(target, provider);
        let mut accumulator = accumulator::ExternalReferenceAccumulator::new(&mut authority);
        for role in [
            ExternalHirReferenceRoleV1::SignatureDependency,
            ExternalHirReferenceRoleV1::DefaultDependency,
            ExternalHirReferenceRoleV1::ConstType,
        ] {
            assert!(accumulator.observe(target, role).unwrap());
        }
        let references = accumulator
            .finish::<AuthorityError>(&mut BudgetMeter::new(DecodeLimits::default()))
            .unwrap();
        assert_eq!(references.records().len(), 1);
        let record = references.get(target).unwrap();
        assert_eq!(record.origin(), provider);
        assert_eq!(
            record.roles().roles(),
            &[
                ExternalHirReferenceRoleV1::SignatureDependency,
                ExternalHirReferenceRoleV1::DefaultDependency,
                ExternalHirReferenceRoleV1::ConstType,
            ]
        );
        assert!(record.witnesses().is_empty());
        record.validate_semantics(&mut authority).unwrap();

        let forged = ExternalHirReferenceV1::try_new(
            cone("false-provider"),
            target,
            record.roles().clone(),
            record.witnesses().clone(),
            Default::default(),
        )
        .unwrap();
        assert!(matches!(
            forged.validate_semantics(&mut authority),
            Err(ExternalHirReferenceSemanticValidationError::OriginMismatch {
                target: actual, expected, ..
            }) if actual == target && expected == provider
        ));
    }
}

#[test]
fn builtin_default_dependencies_still_require_an_actual_provider() {
    let target = ExternalHirTargetV1::Nominal(NominalDeclarationOwner::Concrete(
        CoreBuiltinNominal::Unit.identity_record().id(),
    ));
    let mut authority = Authority::new(cone("consumer"));
    let mut accumulator = accumulator::ExternalReferenceAccumulator::new(&mut authority);
    assert!(matches!(
        accumulator.observe(target, ExternalHirReferenceRoleV1::DefaultDependency),
        Err(ExternalHirReferenceProductionError::TargetOrigin { target: actual, .. })
            if actual == target
    ));
    let record = ExternalHirReferenceV1::try_new(
        CoreBuiltinNominal::Unit.declaration_key().origin(),
        target,
        CanonicalExternalHirReferenceRolesV1::try_new(vec![
            ExternalHirReferenceRoleV1::DefaultDependency,
        ])
        .unwrap(),
        CanonicalDependencyBindingWitnessesV1::try_new(Vec::new()).unwrap(),
        Default::default(),
    )
    .unwrap();
    assert!(matches!(
        record.validate_semantics(&mut authority),
        Err(ExternalHirReferenceSemanticValidationError::TargetOrigin { target: actual, .. })
            if actual == target
    ));
}

#[test]
fn builtin_default_dependencies_do_not_grant_same_name_or_provider_exemptions() {
    let ordinary = cone("provider");
    for (provider, name) in [
        (ordinary, "Unit"),
        (ordinary, "Any"),
        (ConeIdentity::CORE, "Other"),
        (ConeIdentity::CORE, "Unit"),
    ] {
        let target = ExternalHirTargetV1::from(nominal(provider, name));
        let mut authority = Authority::new(cone("consumer")).with_origin(target, provider);
        let mut accumulator = accumulator::ExternalReferenceAccumulator::new(&mut authority);
        accumulator
            .observe(target, ExternalHirReferenceRoleV1::DefaultDependency)
            .unwrap();
        assert!(matches!(
            accumulator.finish::<AuthorityError>(&mut BudgetMeter::new(DecodeLimits::default())),
            Err(ExternalHirReferenceProductionError::MissingWitnessUse {
                target: actual, role: ExternalHirReferenceRoleV1::DefaultDependency,
            }) if actual == target
        ));
        assert_eq!(
            ExternalHirReferenceV1::try_new(
                provider,
                target,
                CanonicalExternalHirReferenceRolesV1::try_new(vec![
                    ExternalHirReferenceRoleV1::DefaultDependency,
                ])
                .unwrap(),
                CanonicalDependencyBindingWitnessesV1::try_new(Vec::new()).unwrap(),
                Default::default(),
            ),
            Err(crate::ExternalHirReferenceBuildError::MissingWitness)
        );
    }
}
