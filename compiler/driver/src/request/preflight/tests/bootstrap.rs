use super::*;
use crate::TrustedCoreArtifactInput;

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
    let output = TrustedCoreBootstrapHirOutput::lower(&parsed).unwrap();
    assert!(matches!(
        output.output_kind(),
        scoop_hir::ConeOutputKind::Library
    ));
    assert_eq!(
        output.production_section().output_contract(),
        &scoop_hir::HirOutputContractV1::Library
    );
    assert!(matches!(
        output.production_section().core_interface(),
        scoop_hir::CoreHirInterfaceBranchV1::Core(_)
    ));
    assert_eq!(
        output.foundation(),
        &scoop_hir::CanonicalHirFoundation::from_modules(
            &output.hir().export,
            &output.hir().local,
            &output.hir().native_boundary_types,
        )
        .unwrap()
    );
    let production_bytes = scoop_wire::encode(output.production_section()).unwrap();
    let decoded =
        scoop_wire::decode_canonical::<scoop_hir::DecodedCoreBootstrapInterfaceSectionV1>(
            &production_bytes,
            scoop_wire::DecodeLimits::default(),
        )
        .unwrap();
    let odr_free_foundation =
        scoop_hir::OdrFreeHirFoundation::try_new(output.foundation().clone()).unwrap();
    assert_eq!(
        decoded
            .validate_against_strong_foundation(
                scoop_identity::ConeIdentity::CORE,
                &odr_free_foundation,
            )
            .unwrap(),
        output.production_section().clone()
    );
    scoop_hir::CoreHirInterfaceV1::from_core_export(&output.hir().export).unwrap();

    let scoop_hir::CoreHirInterfaceBranchV1::Core(interface) =
        output.production_section().core_interface()
    else {
        panic!("the trusted bootstrap output has a core interface")
    };
    let signatures = interface
        .callable_targets()
        .targets()
        .iter()
        .filter_map(|target| {
            let scoop_hir::CoreHirCallableCapabilityV1::ParamFreeCandidate(signature) =
                target.capability()
            else {
                return None;
            };
            let scoop_hir::CoreCallableDefinitionV1::Function(definition) = target.definition()
            else {
                panic!("a param-free core callable candidate has a source function definition")
            };
            Some(scoop_mir::CallableSignatureRecord::new(
                scoop_mir::CallableSignatureSubject::Strong(
                    scoop_identity::CallableOwner::Function(definition),
                ),
                signature.clone(),
            ))
        })
        .collect::<Vec<_>>();
    assert!(!signatures.is_empty());

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
            output.production_section(),
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
        output.production_section(),
        &minimal_foundation,
    )
    .unwrap();
    let scoop_mir::CoreMirBridgeBranchV1::Core(minimal_core_bridge) =
        minimal_production.core_bridge()
    else {
        panic!("the trusted bootstrap MIR production has a core bridge")
    };
    assert!(minimal_core_bridge.callable_targets().is_empty());

    let mut partial_foundation = scoop_mir::CanonicalMirFoundation::empty();
    partial_foundation
        .set_callable_signatures(vec![cycle_signature.clone(), signatures[0].clone()])
        .unwrap();
    let partial_foundation = scoop_mir::OdrFreeMirFoundation::try_new(partial_foundation).unwrap();
    let partial_production = scoop_mir_lower::lower_production_section(
        scoop_identity::ConeIdentity::CORE,
        output.production_section(),
        &partial_foundation,
    )
    .unwrap();
    let scoop_mir::CoreMirBridgeBranchV1::Core(partial_core_bridge) =
        partial_production.core_bridge()
    else {
        panic!("the trusted bootstrap MIR production has a core bridge")
    };
    assert_eq!(partial_core_bridge.callable_targets().len(), 1);

    let mut mismatched_signatures = signatures.clone();
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
    mismatched_signatures.push(cycle_signature.clone());
    let mut mismatched_foundation = scoop_mir::CanonicalMirFoundation::empty();
    mismatched_foundation
        .set_callable_signatures(mismatched_signatures)
        .unwrap();
    let mismatched_foundation =
        scoop_mir::OdrFreeMirFoundation::try_new(mismatched_foundation).unwrap();
    assert!(matches!(
        scoop_mir_lower::lower_production_section(
            scoop_identity::ConeIdentity::CORE,
            output.production_section(),
            &mismatched_foundation,
        ),
        Err(scoop_mir_lower::MirProductionLoweringError::CoreCallableSignatureMismatch { .. })
    ));

    let mut mir_foundation = scoop_mir::CanonicalMirFoundation::empty();
    let mut signatures = signatures;
    signatures.push(cycle_signature);
    mir_foundation.set_callable_signatures(signatures).unwrap();
    let mir_foundation = scoop_mir::OdrFreeMirFoundation::try_new(mir_foundation).unwrap();
    assert!(matches!(
        scoop_mir_lower::lower_production_section(
            scoop_identity::ConeIdentity::SINGLE_FILE,
            output.production_section(),
            &mir_foundation,
        ),
        Err(scoop_mir_lower::MirProductionLoweringError::Production(
            scoop_mir::MirProductionBuildError::UnexpectedCoreBridge(
                scoop_identity::ConeIdentity::SINGLE_FILE
            )
        ))
    ));
    let mir_production = scoop_mir_lower::lower_production_section(
        scoop_identity::ConeIdentity::CORE,
        output.production_section(),
        &mir_foundation,
    )
    .unwrap();
    let scoop_mir::CoreMirBridgeBranchV1::Core(core_bridge) = mir_production.core_bridge() else {
        panic!("the trusted bootstrap MIR production has a core bridge")
    };
    assert_eq!(
        core_bridge.callable_targets().len(),
        interface
            .callable_targets()
            .targets()
            .iter()
            .filter(|target| matches!(
                target.capability(),
                scoop_hir::CoreHirCallableCapabilityV1::ParamFreeCandidate(_)
            ))
            .count()
    );

    let candidate_count = interface
        .callable_targets()
        .targets()
        .iter()
        .filter(|target| {
            matches!(
                target.capability(),
                scoop_hir::CoreHirCallableCapabilityV1::ParamFreeCandidate(_)
            )
        })
        .count();
    let expected_shape_roots = interface
        .type_targets()
        .targets()
        .iter()
        .filter(|target| {
            matches!(
                (target.definition(), target.capability()),
                (
                    scoop_hir::CoreTypeDefinitionV1::Type(_),
                    scoop_hir::CoreHirTypeCapabilityV1::ParamFreeStrong(_)
                )
            )
        })
        .count();
    let missing_source_module = scoop_mir_lower::lower(&output.hir().local).unwrap();
    let missing_source_foundation =
        scoop_mir::OdrFreeMirFoundation::from_module(&missing_source_module).unwrap();
    let missing_source_production = scoop_mir_lower::lower_production_section(
        missing_source_module.cone,
        output.production_section(),
        &missing_source_foundation,
    )
    .unwrap();
    assert!(matches!(
        scoop_mir::SingleConeStrongMirInput::try_new(
            missing_source_module,
            missing_source_foundation,
            missing_source_production,
            scoop_mir::CoreShapeSupportSourceInput::NotCore,
            scoop_mir::StrongImportedCoreInput::Unused,
        ),
        Err(scoop_mir::SingleConeStrongMirInputError::CoreShapeSupportSourceBranchMismatch)
    ));
    let real_mir = output.lower_mir().unwrap();
    assert_eq!(real_mir.mir().cone, scoop_identity::ConeIdentity::CORE);
    let scoop_hir::LocalConcreteMaterializationContract::CoreShapeSupport(shape_plan) =
        real_mir.hir().hir().local.materialization()
    else {
        panic!("trusted core MIR retains the authoritative local shape plan")
    };
    assert_eq!(shape_plan.roots().len(), expected_shape_roots);
    for root in shape_plan.roots() {
        let exact = root.exact();
        let mir = real_mir.mir();
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
            root.boxed_value() == scoop_hir::LocalCoreBoxedValueRequirement::Required
        );
    }
    let scoop_mir::CoreMirBridgeBranchV1::Core(real_core_bridge) =
        real_mir.production_section().core_bridge()
    else {
        panic!("the real trusted bootstrap MIR product has a core bridge")
    };
    assert!(candidate_count > 0);
    assert!(real_core_bridge.callable_targets().is_empty());
    assert!(expected_shape_roots > 0);
    assert_eq!(
        real_core_bridge.shape_support_roots().len(),
        expected_shape_roots
    );
    assert_eq!(
        real_mir.materialization_plan().callable_roots().len(),
        real_mir.mir().top_level.len()
    );
    assert_eq!(
        real_mir.materialization_plan().core_shape_support_roots(),
        real_core_bridge.shape_support_roots()
    );
    assert_eq!(
        real_mir.materialization_plan().core_shape_support_sources(),
        shape_plan
            .roots()
            .iter()
            .map(|root| root.declaration().clone())
            .collect::<Vec<_>>()
    );
    assert!(
        !real_mir
            .materialization_plan()
            .source_nominal_shapes()
            .is_empty()
    );
    assert_eq!(
        real_mir
            .materialization_plan()
            .generated_nominal_shapes()
            .len(),
        real_mir.mir().meta.generated_exact_types.len()
    );
    assert_eq!(
        real_mir
            .production_section()
            .strong_callable_bridges()
            .bridges()
            .len(),
        real_mir.mir().meta.callable_signatures.len()
    );
    let expected_lir_shape_roots = real_core_bridge.shape_support_roots().to_vec();
    let real_lir = real_mir
        .lower_lir(scoop_lir::LirTargetProfile::DARWIN_AARCH64)
        .unwrap();
    let scoop_lir::StrongLirCoreShapeSupportPlan::Core(lir_shape_roots) =
        real_lir.core_shape_support()
    else {
        panic!("trusted core LIR retains its validated core shape plan")
    };
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
            .lir()
            .meta
            .layouts
            .iter()
            .any(|(_, layout)| layout.name.starts_with("Option<"))
    );
    assert!(
        !real_lir
            .lir()
            .meta
            .type_descriptors
            .iter()
            .any(
                |(_, descriptor)| descriptor.diagnostic_name.starts_with("Iterable<")
                    || descriptor.diagnostic_name.starts_with("Iterator<")
            )
    );

    let production = real_lir
        .strong_lir_output()
        .build_production_section(
            scoop_identity::ConeCoordinate::reserved_core(),
            scoop_lir::EntryProductionSourceV1::Library,
        )
        .unwrap();
    let scoop_lir::CoreShapeSupportPlanV1::Core(production_shape_support) =
        production.core_shape_support()
    else {
        panic!("trusted core production retains its core shape-support branch")
    };
    assert_eq!(
        production_shape_support.closures().len(),
        expected_shape_roots
    );
    let strong = real_lir.seal_strong_profile().unwrap();
    let hir_counts = strong.hir_foundation().as_canonical().counts();
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
        ExplicitDependencyInputs::new(Vec::new(), Vec::new()).unwrap(),
        TrustedCoreInput::BootstrapSelf,
        target,
        SlibOutputDestination::new(&artifact_path).unwrap(),
        DiagnosticOutputPolicy::Human,
        StageDumpPolicy::None,
    )
    .unwrap();
    let loaded = request.load_preflight(DecodeLimits::default()).unwrap();
    let validated = loaded.validate().unwrap();
    let parsed = validated.parse_current_sources().unwrap();
    let ParsedSingleConeBuildRequest::TrustedCoreBootstrap(parsed) = parsed else {
        panic!("the request retains its trusted bootstrap branch")
    };

    let published = parsed
        .build_and_publish(&sysroot.path().join("temporary"), DecodeLimits::default())
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
        &scoop_slib::ArtifactCapabilityProfile::CROSS_CONE_SEMANTICS_STRONG.id()
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
        ExplicitDependencyInputs::new(Vec::new(), Vec::new()).unwrap(),
        TrustedCoreInput::Artifact(TrustedCoreArtifactInput::new(core_slot.artifact()).unwrap()),
        target,
        SlibOutputDestination::new(&ordinary_artifact_path).unwrap(),
        DiagnosticOutputPolicy::Human,
        StageDumpPolicy::None,
    )
    .unwrap();
    let ordinary_loaded = ordinary_request
        .load_preflight(DecodeLimits::default())
        .unwrap();
    let ordinary_validated = ordinary_loaded.validate().unwrap();
    let ordinary_parsed = ordinary_validated.parse_current_sources().unwrap();
    let ParsedSingleConeBuildRequest::Ordinary(ordinary_parsed) = ordinary_parsed else {
        panic!("the single-file request retains its ordinary branch")
    };
    let ordinary_published = ordinary_parsed
        .build_and_publish(
            &sysroot.path().join("ordinary-temporary"),
            DecodeLimits::default(),
        )
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
