use super::*;

pub(super) fn artifact(
    manifest_sections: Vec<ManifestSection>,
    lir_sections: Vec<MetadataSection>,
) -> Vec<u8> {
    build_artifact(manifest_sections, lir_sections, true, true, false, true)
}

pub(in super::super) fn complete_artifact(corrupt_final_image_digest: bool) -> Vec<u8> {
    let mut hir_foundation = scoop_hir::CanonicalHirFoundation::empty();
    hir_foundation
        .set_types(vec![
            scoop_identity::CoreBuiltinNominal::Unit.identity_record(),
        ])
        .unwrap();
    let mut mir_foundation = scoop_mir::CanonicalMirFoundation::empty();
    mir_foundation
        .set_exact_types(vec![
            CborIdentityRecord::from_key(scoop_identity::ExactTypeKey::Nominal(
                scoop_identity::CoreBuiltinNominal::Unit
                    .identity_record()
                    .id(),
            ))
            .unwrap(),
        ])
        .unwrap();
    let (lir_foundation, _) =
        strong_production_fixture(cone().coordinate().clone(), &[ConeIdentity::CORE]);
    let lir_proof = ConeLirFoundation::try_new(cone().identity(), lir_foundation.clone()).unwrap();
    let (strong_production, final_objects, defined_symbols, undefined_symbols) =
        finalized_link_object_fixture();
    let image_patch_offset = final_objects
        .runtime_images()
        .fingerprint()
        .image()
        .image_patch()
        .checked_offset();
    let dependencies = vec![
        DependencyRecord::new(
            ConeCoordinate::reserved_core(),
            HirFingerprint::from_array([1; 32]),
            crate::MirFingerprint::from_array([2; 32]),
            crate::LirFingerprint::from_array([3; 32]),
        )
        .unwrap(),
    ];
    let plan = link_object_plan();
    let member_plan = &plan.scoop_lir_members()[0];
    let link_member = SlibMember::new(
        cone().identity(),
        member_plan.stable_key().clone(),
        member_plan.role().clone(),
        final_objects.objects()[0].bytes().to_vec(),
    )
    .unwrap();
    let link_objects =
        crate::verify_code_link_object_members_v1(final_objects, &[link_member.record().clone()])
            .unwrap();
    let code_projection = crate::verify_single_cone_production_code_projection_v1(
        &cone(),
        &dependencies,
        hir_foundation.counts().sources,
        strong_production.clone(),
        link_objects,
    )
    .unwrap();
    let native = CanonicalNativeExternalRequirementSurfaceV1::from_foundation(
        selection().target(),
        &lir_proof,
    )
    .unwrap();
    let code = crate::compute_code_fingerprint_v1(
        code_projection,
        native,
        defined_symbols,
        undefined_symbols,
    )
    .unwrap();
    let closure = crate::LinkIdentityClosureSectionV1::from_verified_code(&code).unwrap();
    let production_manifest = crate::SingleConeProductionManifestV1::from_verified_code(code);
    let link_member = if corrupt_final_image_digest {
        let mut bytes = link_member.payload().to_vec();
        bytes[usize::try_from(image_patch_offset).unwrap()] ^= 1;
        SlibMember::new(
            cone().identity(),
            member_plan.stable_key().clone(),
            member_plan.role().clone(),
            bytes,
        )
        .unwrap()
    } else {
        link_member
    };

    let hir_sections = vec![
        MetadataSection::new(
            MetadataLocation::Hir,
            hir_identity_foundation_capability(),
            MemberPurposeSet::COMPILE_AND_LINK,
            encode(&hir_foundation).unwrap(),
        )
        .unwrap(),
        MetadataSection::new(
            MetadataLocation::Hir,
            hir_core_bootstrap_interface_capability(),
            MemberPurposeSet::COMPILE,
            empty_hir_library_section(),
        )
        .unwrap(),
    ];
    let mir_sections = vec![
        MetadataSection::new(
            MetadataLocation::Mir,
            mir_identity_foundation_capability(),
            MemberPurposeSet::COMPILE_AND_LINK,
            encode(&mir_foundation).unwrap(),
        )
        .unwrap(),
        MetadataSection::new(
            MetadataLocation::Mir,
            mir_core_bootstrap_bridge_capability(),
            MemberPurposeSet::COMPILE,
            empty_mir_library_section(),
        )
        .unwrap(),
    ];
    let lir_sections = vec![
        MetadataSection::new(
            MetadataLocation::Lir,
            lir_identity_foundation_capability(),
            MemberPurposeSet::COMPILE_AND_LINK,
            encode(&lir_foundation).unwrap(),
        )
        .unwrap(),
        MetadataSection::new(
            MetadataLocation::Lir,
            lir_strong_production_capability(),
            MemberPurposeSet::COMPILE_AND_LINK,
            encode(&strong_production).unwrap(),
        )
        .unwrap(),
        MetadataSection::new(
            MetadataLocation::Lir,
            lir_link_identity_closure_capability(),
            MemberPurposeSet::LINK,
            encode(&closure).unwrap(),
        )
        .unwrap(),
    ];
    let compatibility =
        CompatibilityRecord::new(selection(), ArtifactCapabilityProfile::SINGLE_CONE_STRONG)
            .unwrap();
    let foundation_fingerprints = SemanticFingerprintRecord::from_metadata_sections(
        &compatibility,
        &dependencies,
        &known_sections(&hir_sections),
        &known_sections(&mir_sections),
        &known_sections(&lir_sections),
    )
    .unwrap();
    let semantic = SemanticFingerprintRecord::from_production_manifest(
        foundation_fingerprints.hir(),
        foundation_fingerprints.mir(),
        foundation_fingerprints.lir(),
        &production_manifest,
    );
    let manifest_section = ManifestSection::new(
        manifest_single_cone_production_capability(),
        MemberPurposeSet::LINK,
        encode(&production_manifest).unwrap(),
    )
    .unwrap();
    let mut members = vec![
        SlibMember::new(
            cone().identity(),
            MemberStableKey::HirMetadata,
            SlibMemberRole::HirMetadata,
            encode(&MetadataEnvelope::new(MetadataLocation::Hir, hir_sections).unwrap()).unwrap(),
        )
        .unwrap(),
        SlibMember::new(
            cone().identity(),
            MemberStableKey::MirMetadata,
            SlibMemberRole::MirMetadata,
            encode(&MetadataEnvelope::new(MetadataLocation::Mir, mir_sections).unwrap()).unwrap(),
        )
        .unwrap(),
        SlibMember::new(
            cone().identity(),
            MemberStableKey::LirMetadata,
            SlibMemberRole::LirMetadata,
            encode(&MetadataEnvelope::new(MetadataLocation::Lir, lir_sections).unwrap()).unwrap(),
        )
        .unwrap(),
        link_member,
    ];
    let manifest = BootstrapManifest::new(
        ProducerRecord::new("test").unwrap(),
        compatibility,
        cone(),
        dependencies,
        &members,
        semantic,
        vec![manifest_section],
    )
    .unwrap();
    CanonicalSlibArchive::write_bootstrap(&manifest, std::mem::take(&mut members))
        .unwrap()
        .as_bytes()
        .to_vec()
}

pub(super) fn build_artifact(
    manifest_sections: Vec<ManifestSection>,
    lir_sections: Vec<MetadataSection>,
    include_hir_production: bool,
    include_lir_foundation: bool,
    stale_hir_fingerprint: bool,
    include_link_object: bool,
) -> Vec<u8> {
    let mut hir_foundation = scoop_hir::CanonicalHirFoundation::empty();
    hir_foundation
        .set_types(vec![
            scoop_identity::CoreBuiltinNominal::Unit.identity_record(),
        ])
        .unwrap();
    let mut mir_foundation = scoop_mir::CanonicalMirFoundation::empty();
    mir_foundation
        .set_exact_types(vec![
            CborIdentityRecord::from_key(scoop_identity::ExactTypeKey::Nominal(
                scoop_identity::CoreBuiltinNominal::Unit
                    .identity_record()
                    .id(),
            ))
            .unwrap(),
        ])
        .unwrap();
    let mut hir_sections = vec![
        MetadataSection::new(
            MetadataLocation::Hir,
            hir_identity_foundation_capability(),
            MemberPurposeSet::COMPILE_AND_LINK,
            encode(&hir_foundation).unwrap(),
        )
        .unwrap(),
    ];
    if include_hir_production {
        hir_sections.push(
            MetadataSection::new(
                MetadataLocation::Hir,
                hir_core_bootstrap_interface_capability(),
                MemberPurposeSet::COMPILE,
                empty_hir_library_section(),
            )
            .unwrap(),
        );
    }
    let mir_sections = vec![
        MetadataSection::new(
            MetadataLocation::Mir,
            mir_identity_foundation_capability(),
            MemberPurposeSet::COMPILE_AND_LINK,
            encode(&mir_foundation).unwrap(),
        )
        .unwrap(),
        MetadataSection::new(
            MetadataLocation::Mir,
            mir_core_bootstrap_bridge_capability(),
            MemberPurposeSet::COMPILE,
            empty_mir_library_section(),
        )
        .unwrap(),
    ];
    let mut complete_lir_sections = Vec::new();
    if include_lir_foundation {
        let (foundation, _) =
            strong_production_fixture(cone().coordinate().clone(), &[ConeIdentity::CORE]);
        complete_lir_sections.push(
            MetadataSection::new(
                MetadataLocation::Lir,
                lir_identity_foundation_capability(),
                MemberPurposeSet::COMPILE_AND_LINK,
                encode(&foundation).unwrap(),
            )
            .unwrap(),
        );
    }
    complete_lir_sections.extend(lir_sections);

    let compatibility =
        CompatibilityRecord::new(selection(), ArtifactCapabilityProfile::SINGLE_CONE_STRONG)
            .unwrap();
    let dependencies = vec![
        DependencyRecord::new(
            ConeCoordinate::reserved_core(),
            HirFingerprint::from_array([1; 32]),
            crate::MirFingerprint::from_array([2; 32]),
            crate::LirFingerprint::from_array([3; 32]),
        )
        .unwrap(),
    ];
    let semantic = SemanticFingerprintRecord::from_metadata_sections(
        &compatibility,
        &dependencies,
        &known_sections(&hir_sections),
        &known_sections(&mir_sections),
        &known_sections(&complete_lir_sections),
    )
    .unwrap();
    let semantic = SemanticFingerprintRecord::from_digests(
        if stale_hir_fingerprint {
            HirFingerprint::from_array([9; 32])
        } else {
            semantic.hir()
        },
        semantic.mir(),
        semantic.lir(),
        FingerprintAvailability::Available(CodeFingerprint::from_array([4; 32])),
        FingerprintAvailability::Available(RuntimeImageFingerprint::from_array([5; 32])),
    );

    let hir_envelope = MetadataEnvelope::new(MetadataLocation::Hir, hir_sections).unwrap();
    let mir_envelope = MetadataEnvelope::new(MetadataLocation::Mir, mir_sections).unwrap();
    let lir_envelope = MetadataEnvelope::new(MetadataLocation::Lir, complete_lir_sections).unwrap();
    let mut members = vec![
        SlibMember::new(
            cone().identity(),
            MemberStableKey::HirMetadata,
            SlibMemberRole::HirMetadata,
            encode(&hir_envelope).unwrap(),
        )
        .unwrap(),
        SlibMember::new(
            cone().identity(),
            MemberStableKey::MirMetadata,
            SlibMemberRole::MirMetadata,
            encode(&mir_envelope).unwrap(),
        )
        .unwrap(),
        SlibMember::new(
            cone().identity(),
            MemberStableKey::LirMetadata,
            SlibMemberRole::LirMetadata,
            encode(&lir_envelope).unwrap(),
        )
        .unwrap(),
    ];
    if include_link_object {
        let plan = link_object_plan();
        let member = &plan.scoop_lir_members()[0];
        members.push(
            SlibMember::new(
                cone().identity(),
                member.stable_key().clone(),
                member.role().clone(),
                link_object_bytes(),
            )
            .unwrap(),
        );
    }
    let manifest = BootstrapManifest::new(
        ProducerRecord::new("test").unwrap(),
        compatibility,
        cone(),
        dependencies,
        &members,
        semantic,
        manifest_sections,
    )
    .unwrap();
    CanonicalSlibArchive::write_bootstrap(&manifest, members)
        .unwrap()
        .as_bytes()
        .to_vec()
}

fn known_sections(sections: &[MetadataSection]) -> Vec<MetadataSection> {
    sections
        .iter()
        .filter(|section| {
            crate::CapabilityContractRegistry::contract(section.capability()).is_some()
        })
        .cloned()
        .collect()
}
