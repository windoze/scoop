use scoop_identity::{
    CanonicalIdentifier, CanonicalScoopAbiFunctionSignature, DeclarationScope,
    DefinitionOwnerChain, Effect, ExactCallableSignature, ExactTypeKey,
    GcEffect as CanonicalGcEffect, PackagePath, PersistentExactTypeId, PersistentFunctionId,
    ScoopAbiReturn, SourceDeclarationKey, SourceDeclarationSite,
};
use scoop_wire::{decode_canonical, encode};

use super::*;
use crate::{CallableAbiBuildError, CallingConvention, ExternalCallableRootPlan};
use scoop_identity::StrongCallableDefinitionOwner;

use crate::ConeIdentity;

#[derive(Debug)]
struct AbiEntry<T>(Option<Box<T>>);

impl<T: WireEncode> WireEncode for AbiEntry<T> {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encode_initialization_abi(self.0.as_deref(), encoder)
    }
}

impl WireDecode for AbiEntry<DecodedCallableAbiRecordV1> {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decode_initialization_abi(decoder).map(Self)
    }
}

#[test]
fn initialization_abi_has_no_qualification_or_bridge_product() {
    assert_eq!(
        encode(&AbiEntry::<CallableAbiRecordV1>(None)).unwrap(),
        vec![0x80]
    );
    let abi = initialization_cycle_abi_for_test();
    let mut expected = vec![0x81];
    expected.extend(encode(&abi).unwrap());
    let bytes = encode(&AbiEntry(Some(Box::new(abi)))).unwrap();
    assert_eq!(bytes, expected);
    let decoded: AbiEntry<DecodedCallableAbiRecordV1> = decode_canonical(&bytes).unwrap();
    assert_eq!(encode(&decoded).unwrap(), bytes);
}

#[test]
fn initialization_abi_rejects_retired_sums_and_multiple_records() {
    for bytes in [
        vec![0xa1, 0x00, 0x01],
        vec![0xa2, 0x00, 0x02, 0x01, 0xa1, 0x02],
        vec![0x81, 0xa1, 0x02],
    ] {
        assert!(decode_canonical::<AbiEntry<DecodedCallableAbiRecordV1>>(&bytes,).is_err());
    }
    let error =
        decode_canonical::<AbiEntry<DecodedCallableAbiRecordV1>>(&[0x82, 0, 0]).unwrap_err();
    assert_eq!(
        error.kind(),
        &WireErrorKind::InvalidLength {
            expected: 1,
            actual: 2
        }
    );
}

#[test]
fn initialization_abi_preserves_the_shared_root_plan_contract() {
    let abi = callable(ExternalCallableRootPlan::NoGc).unwrap();
    assert_eq!(abi.root_plan(), ExternalCallableRootPlan::NoGc);
    assert!(matches!(
        callable(ExternalCallableRootPlan::ManagedStatepoint),
        Err(CallableAbiBuildError::RootProtocolMismatch { .. })
    ));
}

#[test]
fn service_absence_does_not_depend_on_the_provider_and_present_records_need_their_definition() {
    for provider in [ConeIdentity::CORE, ConeIdentity::SINGLE_FILE] {
        let foundation =
            OdrFreeLirFoundation::try_new(provider, crate::CanonicalLirFoundation::empty())
                .unwrap();
        let definitions =
            StrongObjectSymbolSurfaceV1::from_odr_free_foundation(&foundation).unwrap();
        validate_initialization_abi(None, &foundation, &definitions).unwrap();
        assert!(
            validate_initialization_abi(
                Some(&initialization_cycle_abi_for_test()),
                &foundation,
                &definitions,
            )
            .is_err()
        );
    }
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
