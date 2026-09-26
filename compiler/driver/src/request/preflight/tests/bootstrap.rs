mod shape_sources;

use super::*;

#[test]
fn real_trusted_core_sources_form_the_bootstrap_hir_interface() {
    let slot = crate::trusted_core::resolve_trusted_core_slot_at(
        &crate::workspace_root().join("sysroot"),
        scoop_lir::ValidatedLirTargetSelection::DARWIN_AARCH64_LLVM_22_1,
    )
    .unwrap();
    let manifest = scoop_manifest::load_cone_manifest(&ManifestRootLocator::cone_directory(
        slot.source_root(),
    ))
    .unwrap();
    let sources = discover_manifest_sources(&manifest).unwrap();
    let parsed = parse_discovered_sources(&sources).unwrap();
    let world = scoop_hir::ImportedSemanticWorld::from_validated_closure(
        parsed.cone(),
        Vec::new(),
        Vec::new(),
    )
    .unwrap();
    let output = crate::request::preflight::current_hir::CurrentConeHirArtifacts::lower(
        scoop_identity::RequestedConeKind::Library,
        &parsed,
        scoop_hir_lower::CoreProtocolInput::CurrentDeclarations,
        &world,
    )
    .unwrap();
    assert!(matches!(
        output.hir.output().output_kind(),
        scoop_hir::ConeOutputKind::Library
    ));
    assert_eq!(
        output.production_section.output_contract(),
        &scoop_hir::HirOutputContractV1::Library
    );
    assert!(
        output
            .production_section
            .compiler_protocol_definitions()
            .is_some()
    );
    let mut expected_foundation =
        scoop_hir::CanonicalHirFoundation::from_type_semantics_output(&output.hir).unwrap();
    expected_foundation
        .complete_cross_cone_interface_source_points(
            output.hir.output().export.module(),
            &output.cross_cone_section,
        )
        .unwrap();
    assert!(output.foundation == expected_foundation);
    assert!(
        !output
            .cross_cone_section
            .nominal_interfaces()
            .records()
            .is_empty()
    );
    assert!(
        !output
            .cross_cone_section
            .callable_interfaces()
            .records()
            .is_empty()
    );
    let production_bytes = scoop_wire::encode(&output.production_section).unwrap();
    let decoded =
        scoop_wire::decode_canonical::<scoop_hir::DecodedCoreBootstrapInterfaceSectionV1>(
            &production_bytes,
        )
        .unwrap();
    let odr_free_foundation =
        scoop_hir::OdrFreeHirFoundation::try_new(output.foundation.clone()).unwrap();
    assert_eq!(
        decoded
            .validate_against_strong_foundation(
                scoop_identity::ConeIdentity::CORE,
                &odr_free_foundation,
            )
            .unwrap(),
        output.production_section.clone()
    );
    scoop_hir::CompilerProtocolDefinitionsV1::from_export(&output.hir.output().export).unwrap();

    let Some(interface) = output.production_section.compiler_protocol_definitions() else {
        panic!("the trusted bootstrap output has a core interface")
    };
    let cycle = interface
        .compiler_protocols()
        .initialization_cycle_thrower();
    let scoop_hir::CoreProtocolCallableDefinitionV1::Function(cycle_definition) =
        cycle.definition()
    else {
        panic!("the initialization-cycle protocol has a source function definition")
    };
    let cycle_signature = scoop_mir::CallableSignatureRecord::new(
        scoop_mir::CallableSignatureSubject::Strong(scoop_identity::CallableOwner::Function(
            cycle_definition,
        )),
        scoop_identity::ExactCallableSignature::new(
            scoop_identity::Effect::Ordinary,
            None,
            vec![interface.string_capability().exact_type()],
            scoop_mir::core_unit_exact_type(),
        ),
    );

    let empty_mir_foundation =
        scoop_mir::OdrFreeMirFoundation::try_new(scoop_mir::CanonicalMirFoundation::empty())
            .unwrap();
    assert!(matches!(
        scoop_mir_lower::lower_production_section(
            scoop_identity::ConeIdentity::CORE,
            &output.production_section,
            &empty_mir_foundation,
        ),
        Err(scoop_mir_lower::MirProductionLoweringError::MissingInitializationCycleThrower)
    ));

    let mut minimal_foundation = scoop_mir::CanonicalMirFoundation::empty();
    minimal_foundation
        .set_callable_signatures(vec![cycle_signature.clone()])
        .unwrap();
    let minimal_foundation = scoop_mir::OdrFreeMirFoundation::try_new(minimal_foundation).unwrap();
    let minimal_production = scoop_mir_lower::lower_production_section(
        scoop_identity::ConeIdentity::CORE,
        &output.production_section,
        &minimal_foundation,
    )
    .unwrap();
    let cycle = minimal_production
        .strong_callable_bridges()
        .initialization_cycle()
        .unwrap();
    assert_eq!(
        cycle.implementation(),
        scoop_identity::CallableOwner::Function(cycle_definition)
    );

    let mut mismatched_signatures = vec![cycle_signature.clone()];
    let expected = mismatched_signatures[0].signature();
    let wrong_effect = match expected.effect() {
        scoop_identity::Effect::Ordinary => scoop_identity::Effect::Suspend,
        scoop_identity::Effect::Suspend => scoop_identity::Effect::Ordinary,
    };
    mismatched_signatures[0] = scoop_mir::CallableSignatureRecord::new(
        mismatched_signatures[0].subject(),
        scoop_identity::ExactCallableSignature::new(
            wrong_effect,
            expected.receiver().into_option(),
            expected.parameters().to_vec(),
            expected.result(),
        ),
    );
    let mut mismatched_foundation = scoop_mir::CanonicalMirFoundation::empty();
    mismatched_foundation
        .set_callable_signatures(mismatched_signatures)
        .unwrap();
    let mismatched_foundation =
        scoop_mir::OdrFreeMirFoundation::try_new(mismatched_foundation).unwrap();
    assert!(matches!(
        scoop_mir_lower::lower_production_section(
            scoop_identity::ConeIdentity::CORE,
            &output.production_section,
            &mismatched_foundation,
        ),
        Err(scoop_mir_lower::MirProductionLoweringError::InitializationCycleSignatureMismatch)
    ));

    let expected_shape_roots = scoop_hir::PublicNominalShapeRequirementsV1::from_shared_surface(
        scoop_identity::ConeIdentity::CORE,
        output.production_section.direct_public_surface(),
        &output.foundation,
        output.cross_cone_section.nominal_interfaces(),
        output.cross_cone_section.callable_interfaces(),
    )
    .unwrap()
    .roots()
    .len();
    let real_mir = output
        .machine_input()
        .lower_selected_mir(scoop_mir::SelectedExternalMirSet::empty(
            scoop_identity::ConeIdentity::CORE,
        ))
        .unwrap();
    assert_eq!(
        real_mir.strong.module().cone,
        scoop_identity::ConeIdentity::CORE
    );
    let shape_plan = output.hir.output().local.materialization();
    assert_eq!(shape_plan.roots().len(), expected_shape_roots);
    for root in shape_plan.roots() {
        let exact = root.exact();
        let mir = real_mir.strong.module();
        assert!(
            mir.meta
                .coroutine_steps
                .iter()
                .any(|(_, step)| { step.identity().result_record().id() == exact })
        );
        assert!(
            mir.meta
                .coroutine_slots
                .iter()
                .any(|(_, slot)| { slot.identity().value_record().id() == exact })
        );
        let boxed = mir.meta.boxed_types.iter().any(|boxed| {
            matches!(
                boxed.identity().generated_type_record().key(),
                scoop_identity::GeneratedNominalKey::BoxedValue { payload }
                    if *payload == exact
            )
        });
        assert_eq!(
            boxed,
            root.boxed_value() == scoop_hir::LocalBoxedValueRequirement::Required
        );
    }
    let cycle = real_mir
        .strong
        .production()
        .strong_callable_bridges()
        .initialization_cycle()
        .unwrap();
    assert_eq!(
        cycle.implementation(),
        scoop_identity::CallableOwner::Function(cycle_definition)
    );
    assert!(expected_shape_roots > 0);
    assert_eq!(
        real_mir.strong.materialization().callable_roots().len(),
        real_mir.strong.module().top_level.len()
    );
    assert_eq!(
        real_mir
            .strong
            .materialization()
            .shape_support()
            .iter()
            .map(|root| root.declaration().clone())
            .collect::<Vec<_>>(),
        shape_plan
            .roots()
            .iter()
            .map(|root| root.declaration().clone())
            .collect::<Vec<_>>()
    );
    assert!(
        !real_mir
            .strong
            .materialization()
            .source_nominal_shapes()
            .is_empty()
    );
    assert_eq!(
        real_mir
            .strong
            .materialization()
            .generated_nominal_shapes()
            .len(),
        real_mir.strong.module().meta.generated_exact_types.len()
    );
    assert_eq!(
        real_mir
            .strong
            .production()
            .strong_callable_bridges()
            .bridges()
            .len(),
        real_mir.strong.module().meta.callable_signatures.len()
    );
    let expected_lir_shape_roots = shape_plan.roots().to_vec();
    let mut pending = scoop_identity::PendingIdentityValidation::new();
    pending
        .register_authority(scoop_identity::ConeIdentity::CORE)
        .unwrap();
    output.foundation.register_identities(&mut pending).unwrap();
    real_mir
        .strong
        .foundation()
        .as_canonical()
        .register_identities(&mut pending)
        .unwrap();
    let identities = pending.finish().unwrap();
    let coordinates = [scoop_identity::ConeCoordinate::reserved_core()];
    let diagnostics =
        scoop_identity::ExactTypeDiagnosticCatalog::try_new(&identities, &coordinates).unwrap();
    let (real_lir, _) = machine::lower_selected_lir(
        &real_mir.strong,
        &real_mir.public,
        scoop_lir_lower::RuntimeStringDescriptor::Local,
        &scoop_lir::SelectedExternalLirSet::empty(scoop_identity::ConeIdentity::CORE),
        scoop_lir::LirTargetProfile::DARWIN_AARCH64,
        &scoop_lir::StrongProductionDependencySelectionV2::empty(
            ConeIdentity::CORE,
            scoop_lir::LirTargetProfile::DARWIN_AARCH64,
        )
        .unwrap(),
        &diagnostics,
    )
    .unwrap();
    let lir_shape_roots = real_lir.shape_support().roots();
    assert_eq!(lir_shape_roots.len(), expected_shape_roots);
    for (root, authority) in lir_shape_roots.iter().zip(&expected_lir_shape_roots) {
        assert_eq!(
            root.source().nominal(),
            scoop_identity::PersistentTypeId::from_source_declaration(root.declaration()).unwrap()
        );
        assert_eq!(root.source().nominal(), authority.source());
        assert_eq!(root.source().exact(), authority.exact());
        assert_eq!(root.source().type_descriptor(), root.source().exact());
        assert_eq!(
            root.coroutine_step().type_descriptor(),
            root.coroutine_step().exact()
        );
        assert_eq!(
            root.coroutine_slot().type_descriptor(),
            root.coroutine_slot().exact()
        );
        if let scoop_lir::StrongLirBoxedValueMaterialization::Available(boxed) = root.boxed_value()
        {
            assert_eq!(boxed.type_descriptor(), boxed.exact());
        }
    }
    let lir_counts = real_lir.foundation().as_canonical().counts();
    assert_eq!(lir_counts.odr_groups, 0);
    assert_eq!(lir_counts.odr_members, 0);
    assert!(
        !real_lir
            .module()
            .meta
            .layouts
            .iter()
            .any(|(_, layout)| layout.name.starts_with("Option<"))
    );
    assert!(
        !real_lir
            .module()
            .meta
            .type_descriptors
            .iter()
            .any(
                |(_, descriptor)| descriptor.diagnostic_name.starts_with("Iterable<")
                    || descriptor.diagnostic_name.starts_with("Iterator<")
            )
    );

    let production = real_lir
        .build_production_section_v2(
            scoop_identity::ConeCoordinate::reserved_core(),
            &[],
            scoop_lir::EntryProductionSourceV1::Library,
            &scoop_lir::StrongProductionDependencySelectionV2::empty(
                ConeIdentity::CORE,
                scoop_lir::LirTargetProfile::DARWIN_AARCH64,
            )
            .unwrap(),
            &[],
        )
        .unwrap();
    let production_shape_support = production.shape_support_plan();
    assert_eq!(
        production_shape_support.closures().len(),
        expected_shape_roots
    );
    let hir_counts = output.foundation.counts();
    assert_eq!(hir_counts.callable_applications, 0);
    assert_eq!(hir_counts.odr_groups, 0);
    assert_eq!(hir_counts.odr_members, 0);
}

#[test]
fn parsed_bootstrap_request_publishes_one_two_view_core_artifact() {
    let sysroot = tempfile::tempdir().unwrap();
    copy_trusted_core_sources(sysroot.path());
    let Ok(target) = scoop_toolchain::ResolvedTargetProfile::resolve_host() else {
        // The target resolver owns host Apple-toolchain qualification.
        // Environments blocked by the system Xcode license gate cannot
        // enter object production, but still compile this closed path.
        return;
    };
    let slot = crate::trusted_core::resolve_trusted_core_slot_at(
        sysroot.path(),
        target.lir_target_selection(),
    )
    .unwrap();
    let artifact_path = slot.artifact().to_path_buf();
    std::fs::create_dir_all(artifact_path.parent().unwrap()).unwrap();
    let request = SingleConeBuildRequest::new(
        CurrentConeInput::Manifest {
            root: ManifestRootLocator::cone_directory(slot.source_root()),
        },
        ExplicitDependencyInputs::new(Vec::new(), Vec::new()),
        TrustedCoreInput::BootstrapSelf,
        target,
        SlibOutputDestination::new(&artifact_path).unwrap(),
        DiagnosticOutputPolicy::Human,
        StageDumpPolicy::None,
    )
    .unwrap();
    let loaded = request.load_preflight().unwrap();
    let validated = loaded.validate().unwrap();
    assert!(
        matches!(validated.current(), ValidatedCurrentConeInput::Manifest { manifest }
        if manifest.identity() == ConeIdentity::CORE)
    );
    assert!(matches!(
        validated.protocols(),
        ValidatedCompilerProtocols::CurrentDeclarations
    ));
    let parsed = validated.parse_current_sources().unwrap();

    let published = parsed
        .build_and_publish(&sysroot.path().join("temporary"))
        .unwrap();

    assert_eq!(published.artifact().path(), artifact_path);
    assert_eq!(
        published.artifact().validation().coordinate(),
        &ConeCoordinate::reserved_core()
    );
    assert_eq!(
        published.artifact().validation().identity(),
        scoop_identity::ConeIdentity::CORE
    );
    assert_eq!(
        published.artifact().validation().kind(),
        scoop_slib::ConeKind::Library
    );
    assert_eq!(
        published.artifact().validation().source_form(),
        scoop_slib::ConeSourceForm::Manifest
    );
    assert_eq!(
        published.artifact().validation().profile(),
        &scoop_slib::ArtifactCapabilityProfile::CROSS_CONE_LAYOUT_STRONG.id()
    );
    assert!(
        published
            .artifact()
            .validation()
            .link_summary()
            .link_object_count()
            > 0
    );

    let ordinary_source = sysroot.path().join("ordinary.scoop");
    std::fs::write(&ordinary_source, "fun main() {}\n").unwrap();
    let target = scoop_toolchain::ResolvedTargetProfile::resolve_host().unwrap();
    let core_slot = crate::trusted_core::resolve_trusted_core_slot_at(
        sysroot.path(),
        target.lir_target_selection(),
    )
    .unwrap();
    let ordinary_artifact_path = sysroot.path().join("ordinary.slib");
    let ordinary_request = SingleConeBuildRequest::new(
        CurrentConeInput::SingleFile {
            source: SingleFileLocator::from_path(&ordinary_source).unwrap(),
        },
        ExplicitDependencyInputs::new(Vec::new(), Vec::new()),
        TrustedCoreInput::Artifact(HostArtifactLocator::new(core_slot.artifact()).unwrap()),
        target,
        SlibOutputDestination::new(&ordinary_artifact_path).unwrap(),
        DiagnosticOutputPolicy::Human,
        StageDumpPolicy::None,
    )
    .unwrap();
    let ordinary_loaded = ordinary_request.load_preflight().unwrap();
    let ordinary_validated = ordinary_loaded.validate().unwrap();
    assert!(matches!(
        ordinary_validated.current(),
        ValidatedCurrentConeInput::SingleFile { .. }
    ));
    assert!(matches!(
        ordinary_validated.protocols(),
        ValidatedCompilerProtocols::Imported(_)
    ));
    let ordinary_parsed = ordinary_validated.parse_current_sources().unwrap();

    let ordinary_published = ordinary_parsed
        .build_and_publish(&sysroot.path().join("ordinary-temporary"))
        .unwrap();

    assert_eq!(ordinary_published.artifact().path(), ordinary_artifact_path);
    assert_eq!(
        ordinary_published.artifact().validation().coordinate(),
        &ConeCoordinate::reserved_single_file()
    );
    assert_eq!(
        ordinary_published.artifact().validation().kind(),
        scoop_slib::ConeKind::Executable
    );
    assert_eq!(
        ordinary_published.artifact().validation().source_form(),
        scoop_slib::ConeSourceForm::SingleFile
    );
}

pub(super) fn copy_trusted_core_sources(sysroot: &Path) {
    let source = crate::workspace_root().join("sysroot/lib/scoop.core");
    let destination = sysroot.join("lib/scoop.core");
    std::fs::create_dir_all(destination.join("src")).unwrap();
    std::fs::copy(source.join("Cone.toml"), destination.join("Cone.toml")).unwrap();
    for entry in std::fs::read_dir(source.join("src")).unwrap() {
        let entry = entry.unwrap();
        assert!(entry.file_type().unwrap().is_file());
        std::fs::copy(
            entry.path(),
            destination.join("src").join(entry.file_name()),
        )
        .unwrap();
    }
}
