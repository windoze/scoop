use scoop_identity::{CoreBuiltinNominal, NominalDeclarationOwner, PendingIdentityValidation};

use super::*;

#[test]
fn builtin_default_dependencies_round_trip_without_source_name_witnesses() {
    for builtin in [CoreBuiltinNominal::Unit, CoreBuiltinNominal::Any] {
        let id = builtin.identity_record().id();
        let target = ExternalHirTargetV1::Nominal(NominalDeclarationOwner::Concrete(id));
        for uses in [
            vec![ExternalHirReferenceRoleV1::DefaultDependency],
            vec![
                ExternalHirReferenceRoleV1::SignatureDependency,
                ExternalHirReferenceRoleV1::DefaultDependency,
                ExternalHirReferenceRoleV1::ConstType,
            ],
        ] {
            let record = ExternalHirReferenceV1::try_new(
                builtin.declaration_key().origin(),
                target,
                roles(&uses),
                witnesses(Vec::new()),
                Default::default(),
            )
            .unwrap();
            let mut identities = PendingIdentityValidation::new();
            identities.register_authority(record.origin()).unwrap();
            identities.register_authority(id).unwrap();
            let decoded: DecodedExternalHirReferenceV1 =
                decode_canonical(&encode(&record).unwrap(), DecodeLimits::default()).unwrap();
            assert_eq!(
                decoded.resolve(&mut identities.finish().unwrap()).unwrap(),
                record
            );
        }
    }
}

#[test]
fn builtin_default_dependencies_do_not_waive_actual_source_name_roles() {
    for builtin in [CoreBuiltinNominal::Unit, CoreBuiltinNominal::Any] {
        let target = ExternalHirTargetV1::Nominal(NominalDeclarationOwner::Concrete(
            builtin.identity_record().id(),
        ));
        for role in [
            ExternalHirReferenceRoleV1::ReexportTarget,
            ExternalHirReferenceRoleV1::AliasTarget,
            ExternalHirReferenceRoleV1::ConcreteSelectedUse,
        ] {
            let mut uses = vec![role, ExternalHirReferenceRoleV1::DefaultDependency];
            uses.sort();
            assert_eq!(
                ExternalHirReferenceV1::try_new(
                    builtin.declaration_key().origin(),
                    target,
                    roles(&uses),
                    witnesses(Vec::new()),
                    Default::default(),
                ),
                Err(ExternalHirReferenceBuildError::MissingWitness)
            );
        }
    }
}

#[test]
fn builtin_default_dependencies_reject_fabricated_binding_witnesses_on_decode() {
    let fixture = Fixture::new();
    for builtin in [CoreBuiltinNominal::Unit, CoreBuiltinNominal::Any] {
        let id = builtin.identity_record().id();
        let malformed = RawReference {
            origin: builtin.declaration_key().origin(),
            target: ExternalHirTargetV1::Nominal(NominalDeclarationOwner::Concrete(id)),
            roles: roles(&[ExternalHirReferenceRoleV1::DefaultDependency]),
            witnesses: witnesses(vec![fixture.first_route.clone()]),
        };
        let mut identities = PendingIdentityValidation::new();
        identities.register_authority(malformed.origin).unwrap();
        identities.register_authority(id).unwrap();
        identities.register_authority(fixture.provider).unwrap();
        identities
            .register_authority(fixture.first_route.terminal().binding())
            .unwrap();
        let decoded: DecodedExternalHirReferenceV1 =
            decode_canonical(&encode(&malformed).unwrap(), DecodeLimits::default()).unwrap();
        assert!(matches!(
            decoded.resolve(&mut identities.finish().unwrap()),
            Err(ExternalHirReferenceResolutionError::Shape(
                ExternalHirReferenceBuildError::UnexpectedWitness
            ))
        ));
    }
}
