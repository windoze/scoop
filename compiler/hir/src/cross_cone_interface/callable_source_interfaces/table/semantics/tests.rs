use scoop_identity::{
    AccessorRole, CallableTemplateOrigin, CanonicalIdentifier, CborIdentityRecord, ConeIdentity,
    DeclarationScope, DefinitionOwnerChain, Effect, GcEffect, PackagePath, PersistentGenericTypeId,
    PersistentPropertyAccessorId, PropertyAccessorKey, PropertyOwner, SourceDeclarationKey,
    SourceDeclarationSite,
};

use super::*;
use crate::{
    CallableImplementationV1, CallableInfixV1, CallableInterfaceRecordV1, CallableModalityV1,
    CallableOperatorRoleV1, CallableSafetyV1, CallableSourceEffectsV1, CallableSourceInterfaceV1,
    CanonicalBinderListV1, CanonicalCallableInterfacesV1, CanonicalCallableSourceParametersV1,
    ExportDefinitionSourceV1, PublicDeclarationOwnerV1, PublicLookupAccessV1,
};

#[test]
fn exact_closure_requires_every_non_accessor_and_ignores_accessors() {
    let first = owner("first");
    let second = owner("second");
    let callables = CanonicalCallableInterfacesV1::try_new(vec![
        callable(first),
        callable(second),
        callable(accessor()),
    ])
    .unwrap();
    let sources =
        CanonicalCallableSourceInterfacesV1::try_new(vec![source(second), source(first)]).unwrap();

    assert_eq!(sources.validate_semantics(&callables, &mut NoFacts), Ok(()));
}

#[test]
fn closure_rejects_missing_and_orphan_source_interfaces() {
    let first = owner("first");
    let second = owner("second");
    let callables =
        CanonicalCallableInterfacesV1::try_new(vec![callable(first), callable(second)]).unwrap();
    let missing = CanonicalCallableSourceInterfacesV1::try_new(vec![source(first)]).unwrap();
    assert_eq!(
        missing.validate_semantics(&callables, &mut NoFacts),
        Err(CallableSourceInterfaceSetSemanticValidationError::MissingSourceInterface(second))
    );

    let orphan_owner = owner("orphan");
    let orphan =
        CanonicalCallableSourceInterfacesV1::try_new(vec![source(first), source(orphan_owner)])
            .unwrap();
    assert!(matches!(
        orphan.validate_semantics(&callables, &mut NoFacts),
        Err(CallableSourceInterfaceSetSemanticValidationError::OrphanSourceInterface {
            owner,
            ..
        }) if owner == orphan_owner
    ));
}

fn callable(declaration: CallableTemplateOrigin) -> CallableInterfaceRecordV1 {
    CallableInterfaceRecordV1::try_new(
        declaration,
        PublicDeclarationOwnerV1::TopLevel,
        CanonicalBinderListV1::try_new(Vec::new()).unwrap(),
        None,
        crate::CanonicalSourceParameterShapesV1::try_new(Vec::new()).unwrap(),
        scoop_identity::SignatureTypeKey::Nominal(
            scoop_identity::CoreBuiltinNominal::Unit
                .identity_record()
                .id(),
        ),
        CallableSourceEffectsV1::try_new(
            Effect::Ordinary,
            CallableSafetyV1::Safe,
            GcEffect::Managed,
            CallableImplementationV1::Scoop,
            CallableOperatorRoleV1::None,
            CallableInfixV1::Ordinary,
        )
        .unwrap(),
        CallableModalityV1::Final,
        PublicLookupAccessV1::DirectOnly,
        crate::CanonicalPersistentIdsV1::empty(),
        Vec::new(),
    )
    .unwrap()
}

fn source(owner: CallableTemplateOrigin) -> CallableSourceInterfaceV1 {
    CallableSourceInterfaceV1::try_new(
        owner,
        CanonicalCallableSourceParametersV1::try_new(Vec::new()).unwrap(),
    )
    .unwrap()
}

fn owner(name: &str) -> CallableTemplateOrigin {
    let record = CborIdentityRecord::from_key(SourceDeclarationKey::function(
        site(),
        CanonicalIdentifier::new(name).unwrap(),
        0,
        None,
        Vec::new(),
    ))
    .unwrap();
    CallableTemplateOrigin::Function(record.id())
}

fn accessor() -> CallableTemplateOrigin {
    let property = CborIdentityRecord::from_key(SourceDeclarationKey::property(
        site(),
        CanonicalIdentifier::new("value").unwrap(),
    ))
    .unwrap();
    CallableTemplateOrigin::Accessor(
        PersistentPropertyAccessorId::from_key(&PropertyAccessorKey::new(
            PropertyOwner::Property(property.id()),
            AccessorRole::Getter,
        ))
        .unwrap(),
    )
}

fn site() -> SourceDeclarationSite {
    SourceDeclarationSite::new(
        ConeIdentity::CORE,
        PackagePath::root(),
        DefinitionOwnerChain::top_level(),
        DeclarationScope::ConeWide,
    )
    .unwrap()
}

struct NoFacts;

impl CallableSourceInterfaceSemanticAuthority<NoFactsError> for NoFacts {
    fn current_cone(&self) -> ConeIdentity {
        ConeIdentity::CORE
    }

    fn validate_array_type(&mut self, _array: PersistentGenericTypeId) -> Result<(), NoFactsError> {
        Err(NoFactsError)
    }

    fn validate_source_parameter_origin(
        &mut self,
        _owner: crate::CallableDeclarationId,
        _position: u32,
        _origin: &ExportDefinitionSourceV1,
    ) -> Result<(), NoFactsError> {
        Err(NoFactsError)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct NoFactsError;

impl std::fmt::Display for NoFactsError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("unexpected semantic authority request")
    }
}

impl std::error::Error for NoFactsError {}
