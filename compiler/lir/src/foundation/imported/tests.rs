use scoop_identity::{
    CallableBodyKey, CanonicalIdentifier, CanonicalScoopAbiFunctionSignature, CborIdentityRecord,
    DeclarationScope, DefinitionAtomRole, DefinitionAtomSubkey, DefinitionOwnerChain, Effect,
    ExactTypeKey, GcEffect as CanonicalGcEffect, LinkageClass, ObjectDefinitionAtomKey,
    ObjectDefinitionPlanKey, PackagePath, PendingIdentityValidation, PersistentExactTypeId,
    PersistentFunctionId, PersistentSymbolKey, PersistentSymbolRequestTable, RuntimeIdentityRecord,
    ScoopAbiReturn, SemanticIdentitySession, SemanticOriginFingerprint, SourceDeclarationKey,
    SourceDeclarationSite, StrongDefinitionEntity, StrongDefinitionRole,
};
use scoop_wire::{DecodeLimits, decode_canonical, encode};

use super::*;
use crate::{OdrFreeLirFoundation, StrongObjectSymbolSurfaceV1};

mod descriptors;

#[test]
fn selected_callable_keeps_imported_body_and_definition_authority() {
    let declaration = SourceDeclarationKey::function(
        SourceDeclarationSite::new(
            ConeIdentity::CORE,
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
            ConeIdentity::CORE,
            StrongDefinitionEntity::callable_body(body.id()),
            StrongDefinitionRole::CallableBody,
        )
        .unwrap(),
    )
    .unwrap();
    let type_descriptor_definition = CborIdentityRecord::from_key(
        ObjectDefinitionPlanKey::strong(
            ConeIdentity::CORE,
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
    let strong_foundation =
        OdrFreeLirFoundation::try_new(ConeIdentity::CORE, canonical.clone()).unwrap();
    let definitions =
        StrongObjectSymbolSurfaceV1::from_odr_free_foundation(&strong_foundation).unwrap();
    let foundation = imported_foundation(canonical.clone(), function, &unit_record);
    let other_foundation = imported_foundation(canonical, function, &unit_record);
    let signature = ExactCallableSignature::new(Effect::Ordinary, None, Vec::new(), unit);
    let abi_signature = CanonicalScoopAbiFunctionSignature::new(
        signature.clone(),
        Vec::new(),
        ScoopAbiReturn::unit_void(),
        CanonicalGcEffect::NoGc,
    )
    .unwrap();
    let core_bridge = crate::CoreLirBridgeV1::new(
        crate::CoreLirInitializationCycleThrowerV1::new(
            target,
            abi_signature,
            crate::CallingConvention::Cdecl,
            crate::CoreExternalCallableRootPlan::NoGc,
        )
        .unwrap(),
    );

    let selected = foundation
        .project_initialization_cycle_thrower(&core_bridge, &definitions, target, signature.clone())
        .unwrap();
    let foreign = other_foundation
        .project_initialization_cycle_thrower(&core_bridge, &definitions, target, signature.clone())
        .unwrap();

    assert!(selected.belongs_to(&foundation, &definitions, &core_bridge));
    assert!(!selected.belongs_to(&other_foundation, &definitions, &core_bridge));
    assert_eq!(
        selected.kind(),
        CoreImportedCallableKind::InitializationCycleThrower
    );
    assert_eq!(selected.target(), target);
    assert_eq!(selected.signature(), &signature);
    assert_eq!(selected.body().persistent(), body.id());
    assert_eq!(selected.expected_symbol(), expected_symbol);
    assert_eq!(selected.required_definition().persistent(), definition.id());

    let runtime_string = foundation
        .project_type_descriptor(&definitions, unit)
        .unwrap();
    let mut selected_set =
        SelectedImportedLirSet::try_new(&foundation, &definitions, &core_bridge, runtime_string)
            .unwrap();
    let id = selected_set.insert(selected.clone()).unwrap();
    assert_eq!(selected_set.insert(selected).unwrap(), id);
    assert_eq!(
        selected_set.callable_for_kind(CoreImportedCallableKind::InitializationCycleThrower),
        Some(id)
    );
    assert_eq!(
        selected_set.callable(id).unwrap().kind(),
        CoreImportedCallableKind::InitializationCycleThrower
    );
    assert_eq!(selected_set.len(), 1);
    assert_eq!(
        selected_set.insert(foreign).unwrap_err(),
        ImportedLirSelectionError::ForeignSelection(
            CoreImportedCallableKind::InitializationCycleThrower
        )
    );
    assert_eq!(selected_set.len(), 1);
}

fn imported_foundation(
    canonical: super::super::CanonicalLirFoundation,
    function: PersistentFunctionId,
    exact: &CborIdentityRecord<PersistentExactTypeId, ExactTypeKey>,
) -> ImportedLirFoundation {
    let decoded = decode_canonical::<super::super::DecodedLirFoundation>(
        &encode(&canonical).unwrap(),
        DecodeLimits::default(),
    )
    .unwrap();
    let mut pending = PendingIdentityValidation::new();
    pending.register_authority(ConeIdentity::CORE).unwrap();
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
    >(&encode(exact).unwrap(), DecodeLimits::default())
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
            ConeIdentity::CORE,
            SemanticOriginFingerprint::new([1; 32], [2; 32], [3; 32]),
            &identities,
        )
        .unwrap()
        .into_parts();
    ImportedLirFoundation {
        canonical,
        identities: imported,
    }
}
