use scoop_identity::{
    CanonicalIdentifier, CanonicalScoopAbiFunctionSignature, DeclarationScope,
    DefinitionOwnerChain, Effect, ExactCallableSignature, ExactTypeKey,
    GcEffect as CanonicalGcEffect, PackagePath, PersistentExactTypeId, PersistentFunctionId,
    ScoopAbiReturn, SourceDeclarationKey, SourceDeclarationSite,
};
use scoop_wire::{DecodeLimits, decode_canonical, encode};

use super::*;
use crate::{CallingConvention, ExternalCallableRootPlan};
use scoop_identity::StrongCallableDefinitionOwner;

#[test]
fn empty_branches_have_fixed_closed_wire() {
    assert_eq!(
        encode(&CoreLirBridgeBranchV1::NotCore).unwrap(),
        vec![0xa1, 0x00, 0x01]
    );
    let encoded = encode(&CoreLirBridgeBranchV1::Core(CoreLirBridgeV1::new(
        core_lir_cycle_thrower_for_test(),
    )))
    .unwrap();
    let decoded =
        decode_canonical::<DecodedCoreLirBridgeBranchV1>(&encoded, DecodeLimits::default())
            .unwrap();
    assert_eq!(encode(&decoded).unwrap(), encoded);
}

#[test]
fn decoded_branch_rejects_unknown_incomplete_and_extended_shapes() {
    for bytes in [
        vec![0xa1, 0x00, 0x03],
        vec![0xa1, 0x00, 0x02],
        vec![0xa2, 0x00, 0x01, 0x01, 0x80],
    ] {
        assert!(
            decode_canonical::<DecodedCoreLirBridgeBranchV1>(&bytes, DecodeLimits::default())
                .is_err()
        );
    }
}

#[test]
fn reader_rejects_the_removed_core_callable_table() {
    let mut bytes = encode(&CoreLirBridgeBranchV1::Core(CoreLirBridgeV1::new(
        core_lir_cycle_thrower_for_test(),
    )))
    .unwrap();
    assert_eq!(&bytes[..5], &[0xa2, 0x00, 0x02, 0x01, 0xa1]);
    bytes.splice(4..5, [0xa2, 0x01, 0x80]);
    assert!(
        decode_canonical::<DecodedCoreLirBridgeBranchV1>(&bytes, DecodeLimits::default(),).is_err()
    );
}

#[test]
fn initialization_protocol_bridge_binds_the_root_plan() {
    let bridge = callable(ExternalCallableRootPlan::NoGc).unwrap();
    assert_eq!(bridge.root_plan(), ExternalCallableRootPlan::NoGc);
    assert!(matches!(
        callable(ExternalCallableRootPlan::ManagedStatepoint),
        Err(CallableAbiBuildError::RootProtocolMismatch { .. })
    ));
}

#[test]
fn branch_must_match_the_producer_kind() {
    let foundation = OdrFreeLirFoundation::try_new(
        ConeIdentity::SINGLE_FILE,
        crate::CanonicalLirFoundation::empty(),
    )
    .unwrap();
    let definitions = StrongObjectSymbolSurfaceV1::from_odr_free_foundation(&foundation).unwrap();
    assert!(matches!(
        CoreLirBridgeBranchV1::Core(CoreLirBridgeV1::new(core_lir_cycle_thrower_for_test()),)
            .validate_against(&foundation, &definitions),
        Err(CoreLirBridgeBuildError::ProducerBranchMismatch)
    ));
    let core_foundation =
        OdrFreeLirFoundation::try_new(ConeIdentity::CORE, crate::CanonicalLirFoundation::empty())
            .unwrap();
    let core_definitions =
        StrongObjectSymbolSurfaceV1::from_odr_free_foundation(&core_foundation).unwrap();
    assert!(matches!(
        CoreLirBridgeBranchV1::NotCore.validate_against(&core_foundation, &core_definitions),
        Err(CoreLirBridgeBuildError::ProducerBranchMismatch)
    ));
}

fn callable(
    root_plan: ExternalCallableRootPlan,
) -> Result<CallableAbiRecordV1, CallableAbiBuildError> {
    let declaration = declaration();
    let function = PersistentFunctionId::from_source_declaration(&declaration).unwrap();
    let unit = PersistentExactTypeId::from_key(&ExactTypeKey::Nominal(
        scoop_identity::CoreBuiltinNominal::Unit
            .identity_record()
            .id(),
    ))
    .unwrap();
    let signature = ExactCallableSignature::new(Effect::Ordinary, None, Vec::new(), unit);
    let abi = CanonicalScoopAbiFunctionSignature::new(
        signature,
        Vec::new(),
        ScoopAbiReturn::unit_void(),
        CanonicalGcEffect::NoGc,
    )
    .unwrap();
    CallableAbiRecordV1::new(
        scoop_identity::ConeIdentity::CORE,
        StrongCallableDefinitionOwner::Function(function),
        abi,
        CallingConvention::Cdecl,
        root_plan,
    )
}

fn declaration() -> SourceDeclarationKey {
    SourceDeclarationKey::function(
        SourceDeclarationSite::new(
            ConeIdentity::CORE,
            PackagePath::root(),
            DefinitionOwnerChain::top_level(),
            DeclarationScope::ConeWide,
        )
        .unwrap(),
        CanonicalIdentifier::new("exported").unwrap(),
        0,
        None,
        Vec::new(),
    )
}
