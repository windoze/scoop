use scoop_identity::{
    CallableBodyKey, CanonicalIdentifier, CanonicalScoopAbiFunctionSignature, CborIdentityRecord,
    DeclarationScope, DefinitionAtomRole, DefinitionAtomSubkey, DefinitionOwnerChain, Effect,
    ExactTypeKey, GcEffect as CanonicalGcEffect, LinkageClass, ObjectDefinitionAtomKey,
    ObjectDefinitionPlanKey, PackagePath, PendingIdentityValidation, PersistentExactTypeId,
    PersistentFunctionId, PersistentSymbolKey, PersistentSymbolRequest,
    PersistentSymbolRequestTable, RuntimeIdentityRecord, ScoopAbiReturn, SemanticIdentitySession,
    SemanticOriginFingerprint, SourceDeclarationKey, SourceDeclarationSite, StrongDefinitionEntity,
    StrongDefinitionRole,
};
use scoop_wire::{decode_canonical, encode};

use super::*;
use crate::{OdrFreeLirFoundation, StrongObjectSymbolSurfaceV1};

#[test]
fn selected_callable_keeps_actual_provider_body_and_definition_authority() {
    let ordinary = scoop_identity::ConeCoordinate::new("tests", "initialization", "1.0.0")
        .unwrap()
        .identity()
        .unwrap();
    for provider in [ConeIdentity::CORE, ordinary] {
        check_selected_callable(provider);
    }
}

fn check_selected_callable(provider: ConeIdentity) {
    let declaration = SourceDeclarationKey::function(
        SourceDeclarationSite::new(
            provider,
            PackagePath::root(),
            DefinitionOwnerChain::top_level(),
            DeclarationScope::ConeWide,
        )
        .unwrap(),
        CanonicalIdentifier::new("run").unwrap(),
        0,
        None,
        Vec::new(),
    );
    let function = PersistentFunctionId::from_source_declaration(&declaration).unwrap();
    let target = StrongCallableDefinitionOwner::Function(function);
    let body = RuntimeIdentityRecord::from_key(&CallableBodyKey::strong(target)).unwrap();
    let unit_record = CborIdentityRecord::from_key(ExactTypeKey::Nominal(
        scoop_identity::CoreBuiltinNominal::Unit
            .identity_record()
            .id(),
    ))
    .unwrap();
    let unit = unit_record.id();
    let definition = CborIdentityRecord::from_key(
        ObjectDefinitionPlanKey::strong(
            provider,
            StrongDefinitionEntity::callable_body(body.id()),
            StrongDefinitionRole::CallableBody,
        )
        .unwrap(),
    )
    .unwrap();
    let type_descriptor_definition = CborIdentityRecord::from_key(
        ObjectDefinitionPlanKey::strong(
            provider,
            StrongDefinitionEntity::exact_type(unit),
            StrongDefinitionRole::TypeDescriptor,
        )
        .unwrap(),
    )
    .unwrap();
    let mut canonical = super::super::CanonicalLirFoundation::empty();
    canonical.set_materialized_exact_types(vec![unit]).unwrap();
    canonical.set_callable_bodies(vec![body.clone()]).unwrap();
    canonical
        .set_definition_plans(vec![definition.clone(), type_descriptor_definition.clone()])
        .unwrap();
    canonical
        .set_definition_atoms(vec![
            CborIdentityRecord::from_key(ObjectDefinitionAtomKey::new(
                definition.id(),
                DefinitionAtomRole::Primary,
                DefinitionAtomSubkey::Singleton,
            ))
            .unwrap(),
            CborIdentityRecord::from_key(ObjectDefinitionAtomKey::new(
                definition.id(),
                DefinitionAtomRole::EhFrame,
                DefinitionAtomSubkey::Singleton,
            ))
            .unwrap(),
            CborIdentityRecord::from_key(ObjectDefinitionAtomKey::new(
                type_descriptor_definition.id(),
                DefinitionAtomRole::Primary,
                DefinitionAtomSubkey::Singleton,
            ))
            .unwrap(),
        ])
        .unwrap();
    let expected_symbol = PersistentSymbolRequest::new(
        PersistentSymbolKey::CallableBody(body.id()),
        LinkageClass::ConeStrong,
    )
    .unwrap();
    let string_symbol = PersistentSymbolRequest::new(
        PersistentSymbolKey::TypeDescriptor(unit),
        LinkageClass::ConeStrong,
    )
    .unwrap();
    canonical.set_symbol_requests(
        PersistentSymbolRequestTable::new(vec![expected_symbol, string_symbol]).unwrap(),
    );
    let strong_foundation = OdrFreeLirFoundation::try_new(provider, canonical.clone()).unwrap();
    let definitions =
        StrongObjectSymbolSurfaceV1::from_odr_free_foundation(&strong_foundation).unwrap();
    let foundation = imported_foundation(provider, canonical.clone(), function, &unit_record);
    let other_foundation = imported_foundation(provider, canonical.clone(), function, &unit_record);
    let signature = ExactCallableSignature::new(Effect::Ordinary, None, Vec::new(), unit);
    let abi_signature = CanonicalScoopAbiFunctionSignature::new(
        signature.clone(),
        Vec::new(),
        ScoopAbiReturn::unit_void(),
        CanonicalGcEffect::NoGc,
    )
    .unwrap();
    let abi = crate::CallableAbiRecordV1::new(
        provider,
        target,
        abi_signature,
        crate::CallingConvention::Cdecl,
        crate::ExternalCallableRootPlan::NoGc,
    )
    .unwrap();

    let selected = foundation
        .project_initialization_cycle_thrower(&abi, &definitions, target, signature.clone())
        .unwrap();
    let foreign = other_foundation
        .project_initialization_cycle_thrower(&abi, &definitions, target, signature.clone())
        .unwrap();

    assert_eq!(selected, foreign);
    assert_eq!(selected.bridge().target(), target);
    assert_eq!(selected.bridge().abi_signature().signature(), &signature);
    assert_eq!(selected.bridge().expected_symbol(), expected_symbol);
    assert_eq!(selected.bridge().required_definition(), definition.id());
    let other_provider = if provider == ConeIdentity::CORE {
        ConeIdentity::SINGLE_FILE
    } else {
        ConeIdentity::CORE
    };
    let wrong_provider = crate::CallableAbiRecordV1::new(
        other_provider,
        target,
        abi.abi_signature().clone(),
        abi.calling_convention(),
        abi.root_plan(),
    )
    .unwrap();
    assert!(matches!(
        foundation.project_initialization_cycle_thrower(
            &wrong_provider,
            &definitions,
            target,
            signature.clone()
        ),
        Err(ImportedLirCallableProjectionError::Callable(
            crate::CallableAbiValidationError::ContractMismatch
        )),
    ));
    let wrong_signature = ExactCallableSignature::new(Effect::Suspend, None, Vec::new(), unit);
    assert!(matches!(
        foundation.project_initialization_cycle_thrower(
            &abi,
            &definitions,
            target,
            wrong_signature
        ),
        Err(ImportedLirCallableProjectionError::SignatureMismatch(
            CoreImportedCallableKind::InitializationCycleThrower
        )),
    ));
    let empty = imported_foundation(
        provider,
        CanonicalLirFoundation::empty(),
        function,
        &unit_record,
    );
    assert!(matches!(
        empty.project_initialization_cycle_thrower(&abi, &definitions, target, signature.clone()),
        Err(ImportedLirCallableProjectionError::MissingBody(actual)) if actual == body.id(),
    ));
    canonical.set_definition_plans(Vec::new()).unwrap();
    canonical.set_definition_atoms(Vec::new()).unwrap();
    let without_definition = imported_foundation(provider, canonical, function, &unit_record);
    assert!(matches!(
        without_definition.project_initialization_cycle_thrower(
            &abi, &definitions, target, signature.clone()),
        Err(ImportedLirCallableProjectionError::MissingDefinition(actual)) if actual == definition.id(),
    ));
    assert!(matches!(
        crate::SelectedExternalLirSet::empty(provider).with_initialization_cycle(selected.clone()),
        Err(crate::SelectedExternalLirSetBuildError::SelectedCurrentProvider { provider: actual })
            if actual == provider,
    ));
    let duplicate_role = crate::SelectedExternalLirSet::empty(ConeIdentity::SINGLE_FILE)
        .with_initialization_cycle(selected.clone())
        .unwrap();
    assert!(matches!(
        duplicate_role.with_initialization_cycle(selected.clone()),
        Err(crate::SelectedExternalLirSetBuildError::DuplicateInitializationCycle),
    ));
    let duplicate_target = crate::SelectedExternalLirSet::try_from_callables(
        ConeIdentity::SINGLE_FILE,
        vec![selected.clone()],
    )
    .unwrap();
    assert!(matches!(
        duplicate_target.with_initialization_cycle(selected.clone()),
        Err(crate::SelectedExternalLirSetBuildError::DuplicateCallable { provider: actual, .. })
            if actual == provider,
    ));
    let selected_set = crate::SelectedExternalLirSet::empty(ConeIdentity::SINGLE_FILE)
        .with_initialization_cycle(selected)
        .unwrap();
    drop(foundation);
    drop(other_foundation);
    drop(definitions);
    let id = selected_set.initialization_cycle().unwrap();
    let retained = selected_set.callable(id).unwrap();
    assert_eq!(retained.role(), crate::CallableRole::InitializationCycle);
    assert_eq!(retained.bridge(), foreign.bridge());
    assert_eq!(selected_set.len(), 1);
    assert_eq!(selected_set.dependency_callables().count(), 0);
}

fn imported_foundation(
    provider: ConeIdentity,
    canonical: super::super::CanonicalLirFoundation,
    function: PersistentFunctionId,
    exact: &CborIdentityRecord<PersistentExactTypeId, ExactTypeKey>,
) -> ImportedLirFoundation {
    let decoded =
        decode_canonical::<super::super::DecodedLirFoundation>(&encode(&canonical).unwrap())
            .unwrap();
    let mut pending = PendingIdentityValidation::new();
    pending.register_authority(provider).unwrap();
    pending.register_authority(function).unwrap();
    pending
        .register_authority(
            scoop_identity::CoreBuiltinNominal::Unit
                .identity_record()
                .id(),
        )
        .unwrap();
    let decoded_exact = decode_canonical::<
        scoop_identity::DecodedCborIdentityRecord<
            PersistentExactTypeId,
            scoop_identity::DecodedExactTypeKey,
        >,
    >(&encode(exact).unwrap())
    .unwrap();
    pending
        .register(scoop_identity::IdentityLayer::Hir, &decoded_exact)
        .unwrap();
    decoded.register_identities(&mut pending).unwrap();
    pending.resolve(&decoded_exact).unwrap();
    decoded.resolve_identities(&mut pending).unwrap();
    let identities = pending.finish().unwrap();
    let mut session = SemanticIdentitySession::new();
    let (_, _, imported) = session
        .import(
            provider,
            SemanticOriginFingerprint::new([1; 32], [2; 32], [3; 32]),
            &identities,
        )
        .unwrap()
        .into_parts();
    ImportedLirFoundation {
        canonical: canonical.into(),
        identities: imported,
    }
}
