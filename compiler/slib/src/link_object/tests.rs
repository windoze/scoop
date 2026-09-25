use scoop_identity::{
    CallableBodyKey, CallbackParameterIndex, CanonicalCAbiFunctionSignature, CanonicalCAbiReturn,
    CanonicalCAbiSignatureFingerprintRecord, CanonicalIdentifier, CborIdentityRecord, ConeIdentity,
    DeclarationScope, DefinitionOwnerChain, GeneratedBridgeUnitKey, ObjectDefinitionPlanId,
    ObjectDefinitionPlanKey, PackagePath, PersistentFunctionId, RuntimeIdentityRecord,
    SourceDeclarationKey, SourceDeclarationSite, StrongCallableDefinitionOwner,
    StrongDefinitionEntity, StrongDefinitionRole,
};
use scoop_wire::{decode_canonical, encode};

use super::*;

#[test]
fn scoop_lir_units_are_nonempty_unique_and_canonical() {
    let first = definition_plan(1);
    let second = definition_plan(2);

    assert_eq!(
        CanonicalScoopLirObjectUnitSetV1::new(Vec::new()),
        Err(ObjectUnitSetError::Empty(ObjectUnitKind::ScoopLir))
    );
    assert_eq!(
        CanonicalScoopLirObjectUnitSetV1::new(vec![first, first]),
        Err(ObjectUnitSetError::Duplicate(ObjectUnitKind::ScoopLir))
    );

    let forward = CanonicalScoopLirObjectUnitSetV1::new(vec![first, second]).unwrap();
    let reversed = CanonicalScoopLirObjectUnitSetV1::new(vec![second, first]).unwrap();
    assert_eq!(forward, reversed);
    assert!(forward.units().windows(2).all(|pair| pair[0] < pair[1]));
    assert_eq!(forward.logical_key().unit_count().get(), 2);
}

#[test]
fn generated_bridge_units_use_a_distinct_domain_and_capability() {
    let unit = CborIdentityRecord::from_key(GeneratedBridgeUnitKey::CallbackTrampoline {
        signature: c_signature(),
        context_index: CallbackParameterIndex::new(0),
    })
    .unwrap()
    .id();
    let generated = CanonicalGeneratedBridgeObjectUnitSetV1::new(vec![unit]).unwrap();
    let lir = CanonicalScoopLirObjectUnitSetV1::new(vec![definition_plan(7)]).unwrap();

    assert_ne!(
        generated.logical_key().unit_set_digest().as_array(),
        lir.logical_key().unit_set_digest().as_array()
    );
    assert_eq!(
        generated.logical_key().member_role(),
        SlibMemberRole::LinkObject {
            target_profile: TargetProfileWireId::darwin_aarch64(),
            object_format: ObjectFormatId::macho_relocatable(),
            verifier_capability: generated_c_bridge_link_object_capability(),
        }
    );
    let MemberStableKey::LinkObject {
        verifier_capability,
        ..
    } = generated.logical_key().member_stable_key().unwrap()
    else {
        panic!("generated object key uses the LinkObject branch")
    };
    assert_eq!(
        verifier_capability,
        generated_c_bridge_link_object_capability()
    );
}

#[test]
fn decoded_logical_key_is_rebuilt_from_the_expected_typed_unit_set() {
    let units = CanonicalScoopLirObjectUnitSetV1::new(vec![definition_plan(3)]).unwrap();
    let bytes = encode(&units.logical_key()).unwrap();
    let decoded: DecodedScoopLirObjectLogicalKeyV1 = decode_canonical(&bytes).unwrap();

    assert_eq!(decoded.validate(&units).unwrap(), units.logical_key());

    let other = CanonicalScoopLirObjectUnitSetV1::new(vec![definition_plan(4)]).unwrap();
    assert_eq!(
        decoded.validate(&other),
        Err(ObjectLogicalKeyValidationError::UnitSetDigestMismatch)
    );

    let zero_count = DecodedScoopLirObjectLogicalKeyV1 {
        unit_count: 0,
        unit_set_digest: scoop_wire::Digest256::from_array(
            *units.logical_key().unit_set_digest().as_array(),
        ),
    };
    assert_eq!(
        zero_count.validate(&units),
        Err(ObjectLogicalKeyValidationError::ZeroUnitCount)
    );
}

#[test]
fn object_logical_keys_have_fixed_canonical_wire() {
    let lir = CanonicalScoopLirObjectUnitSetV1::new(vec![definition_plan(11)]).unwrap();
    let generated = CanonicalGeneratedBridgeObjectUnitSetV1::new(vec![
        CborIdentityRecord::from_key(GeneratedBridgeUnitKey::CallbackTrampoline {
            signature: c_signature(),
            context_index: CallbackParameterIndex::new(1),
        })
        .unwrap()
        .id(),
    ])
    .unwrap();

    assert_eq!(
        hex(&encode(&lir.logical_key()).unwrap()),
        "a201010258205c92e8508804f27d465a5aea2872c64d0c25072cd3cd3cbb55dddca6bd02a921"
    );
    assert_eq!(
        hex(&encode(&generated.logical_key()).unwrap()),
        "a2010102582032e746a2f5f68d824e66d5f769a2703af6483efbe505b7f00d2662bc7e4d770e"
    );
}

#[test]
fn member_plan_freezes_identity_before_object_bytes_exist() {
    let first = definition_plan(20);
    let second = definition_plan(21);
    let forward = PlannedScoopLirObjectMemberV1::new(
        ConeIdentity::SINGLE_FILE,
        CanonicalScoopLirObjectUnitSetV1::new(vec![first, second]).unwrap(),
    )
    .unwrap();
    let reversed = PlannedScoopLirObjectMemberV1::new(
        ConeIdentity::SINGLE_FILE,
        CanonicalScoopLirObjectUnitSetV1::new(vec![second, first]).unwrap(),
    )
    .unwrap();
    let split = PlannedScoopLirObjectMemberV1::new(
        ConeIdentity::SINGLE_FILE,
        CanonicalScoopLirObjectUnitSetV1::new(vec![first]).unwrap(),
    )
    .unwrap();
    let other_cone = PlannedScoopLirObjectMemberV1::new(
        ConeIdentity::CORE,
        CanonicalScoopLirObjectUnitSetV1::new(vec![first, second]).unwrap(),
    )
    .unwrap();

    assert_eq!(forward, reversed);
    assert_ne!(forward.member_id(), split.member_id());
    assert_ne!(forward.member_id(), other_cone.member_id());
    assert_eq!(forward.stable_key(), reversed.stable_key());
    assert_eq!(forward.role(), &forward.units().logical_key().member_role());
}

fn definition_plan(seed: u8) -> ObjectDefinitionPlanId {
    let declaration = SourceDeclarationKey::function(
        SourceDeclarationSite::new(
            ConeIdentity::SINGLE_FILE,
            PackagePath::root(),
            DefinitionOwnerChain::top_level(),
            DeclarationScope::ConeWide,
        )
        .unwrap(),
        CanonicalIdentifier::new(&format!("function{seed}")).unwrap(),
        0,
        None,
        Vec::new(),
    );
    let function = PersistentFunctionId::from_source_declaration(&declaration).unwrap();
    let body = RuntimeIdentityRecord::from_key(&CallableBodyKey::strong(
        StrongCallableDefinitionOwner::Function(function),
    ))
    .unwrap()
    .id();
    ObjectDefinitionPlanId::from_key(
        &ObjectDefinitionPlanKey::strong(
            ConeIdentity::SINGLE_FILE,
            StrongDefinitionEntity::callable_body(body),
            StrongDefinitionRole::CallableBody,
        )
        .unwrap(),
    )
    .unwrap()
}

fn c_signature() -> scoop_identity::CanonicalCAbiSignatureFingerprint {
    CanonicalCAbiSignatureFingerprintRecord::new(CanonicalCAbiFunctionSignature::cdecl(
        Vec::new(),
        CanonicalCAbiReturn::Void,
    ))
    .unwrap()
    .fingerprint()
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
