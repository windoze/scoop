use scoop_identity::{
    CanonicalIdentifier, CanonicalScoopAbiFunctionSignature, ConeIdentity, DeclarationScope,
    DefinitionOwnerChain, Effect, ExactCallableSignature, ExactTypeKey, GcEffect, PackagePath,
    PersistentExactTypeId, PersistentFunctionId, PersistentTypeId, ScoopAbiReturn,
    SourceDeclarationKey, SourceDeclarationSite, SourceNominalKind, StrongCallableDefinitionOwner,
};
use scoop_wire::{DecodeLimits, decode_canonical, encode};

use super::*;

#[test]
fn external_bridge_surface_has_fixed_wire_and_validates_against_typed_authority() {
    let surface = surface().unwrap();
    assert_eq!(surface.producer(), ConeIdentity::SINGLE_FILE);
    let bytes = encode(&surface).unwrap();
    assert_eq!(
        hex(&bytes),
        "82a2000101a601a20001015820134f8e77428aceb2c4829bb079d93ac3275bc15b42f263e0ae0d90d1360cc25a02a401a4010102a1000103800458209480c22b8e3c0c0420acfe39cd82cd47051b0ae9da003202b0e10b24f60d16ff028003a10001040103a201a2000101582059c1afa2adf2d73b49fc3b3f9b6decad325c480961659263ecd9a45041b472da020104010501065820d248b27f46be7dbed9540e390a791abc23ae685f2574030fcaed997b7527e303a2000201a3015820ad3ae7a719e82101f547257b8a8ac185f05531c14504be81962250566a3ee86802a201a20004015820ad3ae7a719e82101f547257b8a8ac185f05531c14504be81962250566a3ee868020103582053d2e2db6fa6ff1affee8d969eb8157363a6063d241bfe4b36e9453d41b0dc5b"
    );

    let decoded: DecodedStrongExternalLirBridgeSurfaceV1 =
        decode_canonical(&bytes, DecodeLimits::default()).unwrap();
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
        assert!(
            decode_canonical::<DecodedStrongExternalLirBridgeSurfaceV1>(
                &bytes,
                DecodeLimits::default(),
            )
            .is_err()
        );
    }
}

#[test]
fn core_bootstrap_cannot_claim_external_bridges() {
    let bridge = StrongExternalLirBridgeV1::TypeDescriptor(
        StrongExternalTypeDescriptorBridgeV1::new(exact_type("String", SourceNominalKind::Class))
            .unwrap(),
    );
    assert!(matches!(
        StrongExternalLirBridgeSurfaceV1::try_new(ConeIdentity::CORE, vec![bridge]),
        Err(StrongExternalLirBridgeBuildError::CoreBootstrapImport)
    ));
}

#[test]
fn reader_does_not_repair_a_changed_protocol() {
    let surface = surface().unwrap();
    let mut decoded: DecodedStrongExternalLirBridgeSurfaceV1 =
        decode_canonical(&encode(&surface).unwrap(), DecodeLimits::default()).unwrap();
    let DecodedStrongExternalLirBridgeV1::Callable(callable) = &mut decoded.bridges[0] else {
        panic!("fixture keeps callable first")
    };
    callable.root_plan = ExternalCallableRootPlan::NoGc;

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
    let callable = StrongExternalCallableBridgeV1::new(
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
    .map_err(StrongExternalLirBridgeBuildError::Contract)?;
    let descriptor =
        StrongExternalTypeDescriptorBridgeV1::new(exact_type("String", SourceNominalKind::Class))
            .map_err(StrongExternalLirBridgeBuildError::Contract)?;
    StrongExternalLirBridgeSurfaceV1::try_new(
        ConeIdentity::SINGLE_FILE,
        vec![
            StrongExternalLirBridgeV1::TypeDescriptor(descriptor),
            StrongExternalLirBridgeV1::Callable(callable),
        ],
    )
}

fn exact_type(name: &str, kind: SourceNominalKind) -> PersistentExactTypeId {
    let source = SourceDeclarationKey::nominal(
        core_site(),
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
