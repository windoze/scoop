use super::*;
use scoop_identity::{
    CallableOwner, CanonicalIdentifier, CanonicalScoopAbiFunctionSignature, ConeCoordinate,
    ConeIdentity, CoreBuiltinNominal, DeclarationScope, DefinitionOwnerChain, Effect,
    ExactCallableSignature, ExactTypeKey, GcEffect, PackagePath, PersistentExactTypeId,
    PersistentFunctionId, ScoopAbiReturn, SourceDeclarationKey, SourceDeclarationSite,
    StrongCallableDefinitionOwner,
};
use scoop_lir::{CallingConvention, ExternalCallableRootPlan};
use scoop_mir::{StrongCallableBridgeSurfaceV1, StrongCallableBridgeV1};

#[test]
fn initialization_abi_presence_follows_the_actual_mir_role_for_every_provider() {
    for provider in providers() {
        let (function, abi) = fixture(provider);
        let empty = StrongCallableBridgeSurfaceV1::try_new(Vec::new()).unwrap();
        let present = surface(function, abi.abi_signature().signature().clone());
        assert_eq!(validate_initialization_abi_relation(&empty, None), Ok(()));
        assert_eq!(
            validate_initialization_abi_relation(&present, Some(&abi)),
            Ok(())
        );
        for (mir, lir) in [(&empty, Some(&abi)), (&present, None)] {
            assert_eq!(
                validate_initialization_abi_relation(mir, lir),
                Err(StrongProfileInitializationAbiRelationError::PresenceMismatch)
            );
        }
    }
}

#[test]
fn initialization_abi_relation_keeps_typed_target_and_every_logical_signature_component() {
    let [core, ordinary] = providers();
    for (provider, other) in [(core, ordinary), (ordinary, core)] {
        let (function, abi) = fixture(provider);
        let (other_function, other_abi) = fixture(other);
        assert_ne!(function, other_function);
        let mir = surface(function, abi.abi_signature().signature().clone());
        assert_eq!(
            validate_initialization_abi_relation(&mir, Some(&other_abi)),
            Err(StrongProfileInitializationAbiRelationError::InitializationCycleMismatch)
        );
        let unit = exact(CoreBuiltinNominal::Unit);
        for signature in [
            ExactCallableSignature::new(Effect::Suspend, None, Vec::new(), unit),
            ExactCallableSignature::new(Effect::Ordinary, Some(unit), Vec::new(), unit),
            ExactCallableSignature::new(Effect::Ordinary, None, vec![unit], unit),
            ExactCallableSignature::new(
                Effect::Ordinary,
                None,
                Vec::new(),
                exact(CoreBuiltinNominal::Any),
            ),
        ] {
            assert_eq!(validate_initialization_abi_relation(&surface(function, signature), Some(&abi)),
                Err(StrongProfileInitializationAbiRelationError::InitializationCycleSignatureMismatch));
        }
    }
}

fn providers() -> [ConeIdentity; 2] {
    [
        ConeIdentity::CORE,
        ConeCoordinate::new("tests", "initialization", "1.0.0")
            .unwrap()
            .identity()
            .unwrap(),
    ]
}

fn exact(nominal: CoreBuiltinNominal) -> PersistentExactTypeId {
    PersistentExactTypeId::from_key(&ExactTypeKey::Nominal(nominal.identity_record().id())).unwrap()
}

fn surface(
    function: PersistentFunctionId,
    signature: ExactCallableSignature,
) -> StrongCallableBridgeSurfaceV1 {
    StrongCallableBridgeSurfaceV1::try_new(vec![StrongCallableBridgeV1::new(
        CallableOwner::Function(function),
        signature,
    )])
    .unwrap()
    .with_initialization_cycle(function)
    .unwrap()
}

fn fixture(provider: ConeIdentity) -> (PersistentFunctionId, CallableAbiRecordV1) {
    let source = SourceDeclarationKey::function(
        SourceDeclarationSite::new(
            provider,
            PackagePath::root(),
            DefinitionOwnerChain::top_level(),
            DeclarationScope::ConeWide,
        )
        .unwrap(),
        CanonicalIdentifier::new("cycle").unwrap(),
        0,
        None,
        Vec::new(),
    );
    let function = PersistentFunctionId::from_source_declaration(&source).unwrap();
    let signature = CanonicalScoopAbiFunctionSignature::new(
        ExactCallableSignature::new(
            Effect::Ordinary,
            None,
            Vec::new(),
            exact(CoreBuiltinNominal::Unit),
        ),
        Vec::new(),
        ScoopAbiReturn::unit_void(),
        GcEffect::Managed,
    )
    .unwrap();
    let abi = CallableAbiRecordV1::new(
        provider,
        StrongCallableDefinitionOwner::Function(function),
        signature,
        CallingConvention::Cdecl,
        ExternalCallableRootPlan::ManagedStatepoint,
    )
    .unwrap();
    (function, abi)
}
