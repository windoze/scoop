use scoop_identity::{
    CanonicalIdentifier, CanonicalScoopAbiFunctionSignature, ConeIdentity, DeclarationScope,
    DefinitionOwnerChain, Effect, ExactCallableSignature, ExactTypeKey, GcEffect, PackagePath,
    PersistentExactTypeId, PersistentFunctionId, PersistentTypeId, ScoopAbiReturn,
    SourceDeclarationKey, SourceDeclarationSite, SourceNominalKind, StrongCallableDefinitionOwner,
};
use scoop_wire::{decode_canonical, encode};

use super::*;
use crate::{CallingConvention, ExternalCallableRootPlan};

#[test]
fn external_callable_surface_has_fixed_wire() {
    let surface = surface().unwrap();
    assert_eq!(surface.producer(), ConeIdentity::SINGLE_FILE);
    let bytes = encode(&surface).unwrap();
    assert_eq!(
        hex(&bytes),
        "81a2000301a20158205ea5f5e8ff248182c8f8c7e1043caae20f163bcefd34cca4e97d8c6a03bf620d02a201a20001015820134f8e77428aceb2c4829bb079d93ac3275bc15b42f263e0ae0d90d1360cc25a02a601a20001015820134f8e77428aceb2c4829bb079d93ac3275bc15b42f263e0ae0d90d1360cc25a02a401a4010102a1000103800458209480c22b8e3c0c0420acfe39cd82cd47051b0ae9da003202b0e10b24f60d16ff028003a10001040103a201a2000101582059c1afa2adf2d73b49fc3b3f9b6decad325c480961659263ecd9a45041b472da020104010501065820d248b27f46be7dbed9540e390a791abc23ae685f2574030fcaed997b7527e303"
    );

    let decoded: DecodedStrongExternalLirBridgeSurfaceV1 = decode_canonical(&bytes).unwrap();
    assert_eq!(decoded.validate_against(&surface).unwrap(), surface);
}

#[test]
fn external_bridge_reader_rejects_open_sums_and_products() {
    for bytes in [
        vec![0x81, 0xa0],
        vec![0x81, 0xa1, 0x00, 0x01],
        vec![0x81, 0xa2, 0x00, 0x03, 0x01, 0xa0],
        vec![0x81, 0xa2, 0x00, 0x02, 0x01, 0xa2],
    ] {
        assert!(decode_canonical::<DecodedStrongExternalLirBridgeSurfaceV1>(&bytes,).is_err());
    }
}

#[test]
fn external_references_reject_self_imports_for_core_too() {
    let bridge = surface().unwrap().bridges()[0].clone();
    assert!(matches!(
        StrongExternalLirBridgeSurfaceV1::try_new(ConeIdentity::CORE, vec![bridge]),
        Err(StrongExternalLirBridgeBuildError::SelfImport {
            provider: ConeIdentity::CORE
        })
    ));
}

#[test]
fn reader_does_not_repair_a_changed_protocol() {
    let surface = surface().unwrap();
    let StrongExternalLirBridgeV1::Callable(callable) = &surface.bridges()[0];
    let mut bytes = encode(callable.as_ref()).unwrap();
    let root_offset = bytes.len()
        - encode(&callable.bridge().required_definition())
            .unwrap()
            .len()
        - 2;
    assert_eq!(&bytes[root_offset - 1..=root_offset + 1], &[5, 1, 6]);
    bytes[root_offset] = 2;
    let changed = decode_canonical(&bytes).unwrap();
    let mut decoded: DecodedStrongExternalLirBridgeSurfaceV1 =
        decode_canonical(&encode(&surface).unwrap()).unwrap();
    decoded.bridges[0] = DecodedStrongExternalLirBridgeV1::Callable(Box::new(changed));

    assert!(matches!(
        decoded.validate_against(&surface),
        Err(StrongExternalLirBridgeValidationError::SurfaceMismatch)
    ));
}

fn surface() -> Result<StrongExternalLirBridgeSurfaceV1, StrongExternalLirBridgeBuildError> {
    let unit = exact_type("Unit", SourceNominalKind::Object);
    let function = PersistentFunctionId::from_source_declaration(&SourceDeclarationKey::function(
        core_site(),
        CanonicalIdentifier::new("println").unwrap(),
        0,
        None,
        Vec::new(),
    ))
    .unwrap();
    let callable = SelectedDependencyLirCallableV1::new(
        scoop_identity::ConeIdentity::CORE,
        scoop_identity::DependencyCallableDeclarationId::Function(function),
        StrongCallableDefinitionOwner::Function(function),
        CanonicalScoopAbiFunctionSignature::new(
            ExactCallableSignature::new(Effect::Ordinary, None, Vec::new(), unit),
            Vec::new(),
            ScoopAbiReturn::unit_void(),
            GcEffect::Managed,
        )
        .unwrap(),
        CallingConvention::Cdecl,
        ExternalCallableRootPlan::ManagedStatepoint,
    )
    .map_err(StrongExternalLirBridgeBuildError::Callable)?;
    StrongExternalLirBridgeSurfaceV1::try_new(
        ConeIdentity::SINGLE_FILE,
        vec![StrongExternalLirBridgeV1::Callable(Box::new(callable))],
    )
}

fn exact_type(name: &str, kind: SourceNominalKind) -> PersistentExactTypeId {
    exact_type_at(ConeIdentity::CORE, name, kind)
}

fn exact_type_at(
    provider: ConeIdentity,
    name: &str,
    kind: SourceNominalKind,
) -> PersistentExactTypeId {
    let source = SourceDeclarationKey::nominal(
        SourceDeclarationSite::new(
            provider,
            PackagePath::root(),
            DefinitionOwnerChain::top_level(),
            DeclarationScope::ConeWide,
        )
        .unwrap(),
        CanonicalIdentifier::new(name).unwrap(),
        kind,
        0,
    );
    let ty = PersistentTypeId::from_source_declaration(&source).unwrap();
    PersistentExactTypeId::from_key(&ExactTypeKey::Nominal(ty)).unwrap()
}

fn core_site() -> SourceDeclarationSite {
    SourceDeclarationSite::new(
        ConeIdentity::CORE,
        PackagePath::root(),
        DefinitionOwnerChain::top_level(),
        DeclarationScope::ConeWide,
    )
    .unwrap()
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[test]
fn retired_service_tags_cannot_wrap_a_shared_record() {
    let mut bytes = encode(&surface().unwrap()).unwrap();
    assert_eq!(&bytes[..4], &[0x81, 0xa2, 0, 3]);
    for retired in [1, 2] {
        bytes[3] = retired;
        assert!(decode_canonical::<DecodedStrongExternalLirBridgeSurfaceV1>(&bytes).is_err());
    }
}
