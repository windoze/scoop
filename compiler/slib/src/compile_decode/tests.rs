use scoop_hir::CanonicalHirFoundation;
use scoop_identity::{
    CapabilityId, CborIdentityRecord, ConeCoordinate, CoreBuiltinNominal, ExactTypeKey, LayoutKey,
    RepresentationRole,
};
use scoop_lir::{CanonicalLirFoundation, ValidatedLirTargetSelection};
use scoop_mir::CanonicalMirFoundation;
use scoop_wire::{DecodeLimits, encode};

use super::*;
use crate::{
    ConeKind, ConeRecord, ConeSourceForm, IdentityFoundationArtifact,
    IdentityFoundationArtifactInput, ManifestSection, ProducerRecord,
};

#[test]
fn graph_decodes_identity_checks_and_structurally_validates_all_foundation_layers() {
    let selection = ValidatedLirTargetSelection::DARWIN_AARCH64_LLVM_22_1;
    let unit = CoreBuiltinNominal::Unit.identity_record();
    let exact_record = CborIdentityRecord::from_key(ExactTypeKey::Nominal(unit.id())).unwrap();
    let layout_record = CborIdentityRecord::from_key(LayoutKey::new(
        exact_record.id(),
        selection.target().wire_id(),
        RepresentationRole::ManagedValue,
    ))
    .unwrap();
    let mut hir = CanonicalHirFoundation::empty();
    hir.set_types(vec![unit, CoreBuiltinNominal::Any.identity_record()])
        .unwrap();
    let mut mir = CanonicalMirFoundation::empty();
    mir.set_exact_types(vec![exact_record]).unwrap();
    let mut lir = CanonicalLirFoundation::empty();
    lir.set_layouts(vec![layout_record]).unwrap();
    let cone = ConeRecord::new(
        ConeCoordinate::reserved_core(),
        ConeKind::Library,
        ConeSourceForm::Manifest,
    )
    .unwrap();
    let artifact = IdentityFoundationArtifact::write(IdentityFoundationArtifactInput::new(
        ProducerRecord::new("test").unwrap(),
        cone.clone(),
        selection,
        &hir,
        &mir,
        &lir,
    ))
    .unwrap();
    let envelope =
        crate::DecodedSlibEnvelope::open(artifact.as_bytes(), DecodeLimits::default(), selection)
            .unwrap();
    let envelope_usage = envelope.decode_usage();
    let decoded = envelope
        .validate_graph()
        .unwrap()
        .decode_identity_foundations()
        .unwrap();

    assert_eq!(decoded.identity(), cone.identity());
    assert_eq!(encode(decoded.hir_wire()).unwrap(), encode(&hir).unwrap());
    assert_eq!(encode(decoded.mir_wire()).unwrap(), encode(&mir).unwrap());
    assert_eq!(encode(decoded.lir_wire()).unwrap(), encode(&lir).unwrap());
    assert!(decoded.decode_usage().decoded_nodes > envelope_usage.decoded_nodes);
    assert!(decoded.decode_usage().logical_heap_bytes > envelope_usage.logical_heap_bytes);

    let checked = decoded.validate_identities().unwrap();
    assert_eq!(checked.identity(), cone.identity());
    assert_eq!(checked.identity_count(), 5);
    assert_eq!(encode(checked.hir_wire()).unwrap(), encode(&hir).unwrap());
    assert_eq!(encode(checked.mir_wire()).unwrap(), encode(&mir).unwrap());
    assert_eq!(encode(checked.lir_wire()).unwrap(), encode(&lir).unwrap());

    let validated = checked.validate_structure().unwrap();
    assert_eq!(validated.identity(), cone.identity());
    assert_eq!(validated.identity_count(), 5);
    assert_eq!(validated.hir().counts().types, 2);
    assert_eq!(validated.mir().counts().exact_types, 1);
    assert_eq!(validated.lir().counts().layouts, 1);
    assert_eq!(encode(validated.hir()).unwrap(), encode(&hir).unwrap());
    assert_eq!(encode(validated.mir()).unwrap(), encode(&mir).unwrap());
    assert_eq!(encode(validated.lir()).unwrap(), encode(&lir).unwrap());
}

#[test]
fn structural_validation_reports_the_failing_layer() {
    let selection = ValidatedLirTargetSelection::DARWIN_AARCH64_LLVM_22_1;
    let hir = CanonicalHirFoundation::empty();
    let mir = CanonicalMirFoundation::empty();
    let lir = CanonicalLirFoundation::empty();
    let cone = ConeRecord::new(
        ConeCoordinate::reserved_core(),
        ConeKind::Library,
        ConeSourceForm::Manifest,
    )
    .unwrap();
    let artifact = IdentityFoundationArtifact::write(IdentityFoundationArtifactInput::new(
        ProducerRecord::new("test").unwrap(),
        cone,
        selection,
        &hir,
        &mir,
        &lir,
    ))
    .unwrap();
    let checked =
        crate::DecodedSlibEnvelope::open(artifact.as_bytes(), DecodeLimits::default(), selection)
            .unwrap()
            .validate_graph()
            .unwrap()
            .decode_identity_foundations()
            .unwrap()
            .validate_identities()
            .unwrap();

    assert!(matches!(
        checked.validate_structure(),
        Err(FoundationStructureValidationError::Hir(_))
    ));
}

#[test]
fn unknown_compile_required_manifest_capability_stops_foundation_decode() {
    let hir = CanonicalHirFoundation::empty();
    let mir = CanonicalMirFoundation::empty();
    let lir = CanonicalLirFoundation::empty();
    let selection = ValidatedLirTargetSelection::DARWIN_AARCH64_LLVM_22_1;
    let cone = ConeRecord::new(
        ConeCoordinate::reserved_core(),
        ConeKind::Library,
        ConeSourceForm::Manifest,
    )
    .unwrap();
    let capability = CapabilityId::new("org.scoop-lang.test", "compile", 1).unwrap();
    let metadata = crate::IdentityFoundationMetadata::new(&hir, &mir, &lir).unwrap();
    let compatibility = crate::CompatibilityRecord::identity_foundation(selection).unwrap();
    let fingerprints = SemanticFingerprintRecord::identity_foundation(
        &compatibility,
        &[],
        metadata.hir_section(),
        metadata.mir_section(),
        metadata.lir_section(),
    )
    .unwrap();
    let members = metadata.into_members(cone.identity()).unwrap();
    let manifest = crate::BootstrapManifest::new(
        ProducerRecord::new("test").unwrap(),
        compatibility,
        cone,
        Vec::new(),
        &members,
        fingerprints,
        vec![
            ManifestSection::new(capability.clone(), MemberPurposeSet::COMPILE, Vec::new())
                .unwrap(),
        ],
    )
    .unwrap();
    let archive = crate::CanonicalSlibArchive::write_bootstrap(&manifest, members).unwrap();
    let graph =
        crate::DecodedSlibEnvelope::open(archive.as_bytes(), DecodeLimits::default(), selection)
            .unwrap()
            .validate_graph()
            .unwrap();

    assert!(matches!(
        graph.decode_identity_foundations(),
        Err(IdentityFoundationDecodeError::UnknownCompileCapability {
            location: None,
            index: 0,
            capability: actual,
        }) if actual == capability
    ));
}
