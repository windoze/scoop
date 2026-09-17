use super::*;

#[test]
fn dependency_strong_lowering_requires_lir_authority_and_preserves_gc_protocols() {
    for effect in [mir::GcEffect::Managed, mir::GcEffect::NoGc] {
        let (input, selected_lir, expected_target) = dependency_input(effect, effect, false);
        let core = test_imported_core_lir_input(&input);

        assert!(matches!(
            super::super::lower(&input, core, lir::LirTargetProfile::DARWIN_AARCH64,),
            Err(StrongLirLoweringError::MissingImportedDependencyLirAuthority)
        ));

        let output = super::super::lower_with_dependencies(
            &input,
            core,
            super::super::StrongImportedDependencyLirInput::Selected(&selected_lir),
            lir::LirTargetProfile::DARWIN_AARCH64,
        )
        .unwrap();
        let module = output.module();
        assert!(module.meta.core_external_callables.is_empty());
        assert_eq!(module.meta.dependency_external_callables.len(), 1);
        assert_eq!(module.functions.len(), 1);

        let (_, external) = module
            .meta
            .dependency_external_callables
            .iter()
            .next()
            .unwrap();
        assert_eq!(external.target(), expected_target);
        assert_eq!(
            external.gc_effect(),
            match effect {
                mir::GcEffect::Managed => lir::GcEffect::Managed,
                mir::GcEffect::NoGc => lir::GcEffect::NoGc,
            }
        );
        assert!(
            module
                .functions
                .iter()
                .all(|function| function.callable_body.id() != external.body())
        );

        let call = module.functions[0].blocks[module.functions[0].entry]
            .instructions
            .iter()
            .find_map(|instruction| match instruction {
                lir::Instruction::Call { site } => Some(site),
                _ => None,
            })
            .expect("dependency call remains one typed LIR call");
        assert!(matches!(
            call.destination(&module.functions[0].call_targets),
            lir::CallDestination::DependencyExternal(_)
        ));
    }
}

#[test]
fn dependency_strong_lowering_rejects_gc_effect_drift() {
    let (input, selected_lir, _) =
        dependency_input(mir::GcEffect::Managed, mir::GcEffect::NoGc, false);
    let core = test_imported_core_lir_input(&input);

    assert!(matches!(
        super::super::lower_with_dependencies(
            &input,
            core,
            super::super::StrongImportedDependencyLirInput::Selected(&selected_lir),
            lir::LirTargetProfile::DARWIN_AARCH64,
        ),
        Err(
            StrongLirLoweringError::ImportedDependencyLirGcEffectMismatch {
                mir: mir::GcEffect::Managed,
                lir: scoop_identity::GcEffect::NoGc,
                ..
            }
        )
    ));
}

#[test]
fn dependency_strong_lowering_classifies_an_extension_receiver_as_the_first_argument() {
    let (input, selected_lir, _) =
        dependency_input(mir::GcEffect::Managed, mir::GcEffect::Managed, true);
    let core = test_imported_core_lir_input(&input);

    let output = super::super::lower_with_dependencies(
        &input,
        core,
        super::super::StrongImportedDependencyLirInput::Selected(&selected_lir),
        lir::LirTargetProfile::DARWIN_AARCH64,
    )
    .unwrap();
    let (_, external) = output
        .module()
        .meta
        .dependency_external_callables
        .iter()
        .next()
        .unwrap();

    assert_eq!(external.signature().arguments().len(), 1);
    assert!(external.signature().arguments()[0].is_elided());
}

fn dependency_input(
    mir_effect: mir::GcEffect,
    lir_effect: mir::GcEffect,
    has_receiver: bool,
) -> (
    mir::SingleConeStrongMirInput,
    lir::SelectedDependencyLirSet,
    scoop_identity::StrongCallableDefinitionOwner,
) {
    let mut builder = Builder::new();
    let caller = builder.user_fn("caller", Arena::new(), Vec::new());
    let mut module = builder.finish(caller);
    module.output = mir::MirOutput::Library;

    let provider = scoop_identity::ConeCoordinate::new("tests", "dependency-provider", "1.0.0")
        .unwrap()
        .identity()
        .unwrap();
    let declaration = SourceDeclarationKey::function(
        SourceDeclarationSite::new(
            provider,
            PackagePath::root(),
            DefinitionOwnerChain::top_level(),
            DeclarationScope::ConeWide,
        )
        .unwrap(),
        CanonicalIdentifier::new("run").unwrap(),
        0,
        has_receiver
            .then(|| SignatureTypeKey::Nominal(CoreBuiltinNominal::Unit.identity_record().id())),
        Vec::new(),
    );
    let function = PersistentFunctionId::from_source_declaration(&declaration).unwrap();
    let declaration = scoop_identity::DependencyCallableDeclarationId::Function(function);
    let target = scoop_identity::StrongCallableDefinitionOwner::Function(function);
    let unit = module
        .meta
        .source_exact_types
        .get(&mir::Type::Unit)
        .unwrap()
        .identity_record()
        .id();
    let exact = ExactCallableSignature::new(
        Effect::Ordinary,
        has_receiver.then_some(unit),
        Vec::new(),
        unit,
    );

    let initial_foundation = mir::OdrFreeMirFoundation::from_module(&module).unwrap();
    let mir_record =
        mir::SelectedDependencyMirCallableV1::try_new(provider, declaration, target, exact.clone())
            .unwrap();
    let mir_bridge = mir::CrossConeMirBridgeSectionV1::try_new(
        module.cone,
        &initial_foundation,
        Vec::new(),
        vec![mir_record],
    )
    .unwrap();
    let selected_mir = mir::SelectedDependencyMirSet::try_from_bridge(&mir_bridge).unwrap();
    let selected_id = selected_mir.callable_for(provider, declaration).unwrap();
    let imported = module.meta.imported_dependency_callables.alloc(
        selected_mir
            .callable_use(selected_id, mir_effect)
            .expect("selected MIR dependency mints one effect-refined use"),
    );
    let entry = module.functions[caller].body.entry;
    module.functions[caller].body.blocks[entry]
        .statements
        .push(call_stmt(mir::Call {
            target: mir::CallTarget {
                kind: mir::CallKind::Direct,
                callee: mir::Callee::DependencyStrong(imported),
            },
            args: if has_receiver {
                vec![mir::Expr::unit()]
            } else {
                Vec::new()
            },
            pending: mir::CoroutinePendingContext::Root,
        }));

    let foundation = mir::OdrFreeMirFoundation::from_module(&module).unwrap();
    let production = mir::CoreBootstrapBridgeSectionV1::try_new(
        module.cone,
        mir::CoreMirBridgeBranchV1::NotCore,
        mir::EntryMirBridgeBranchV1::Library,
        mir::StrongCallableBridgeSurfaceV1::from_odr_free_foundation(&foundation),
    )
    .unwrap();
    let input = mir::SingleConeStrongMirInput::try_new_with_dependencies(
        module,
        foundation,
        production,
        mir::CoreShapeSupportSourceInput::NotCore,
        mir::StrongImportedCoreInput::Unused,
        mir::StrongImportedDependencyInput::Selected(&selected_mir),
    )
    .unwrap();

    let canonical_effect = match lir_effect {
        mir::GcEffect::Managed => scoop_identity::GcEffect::Managed,
        mir::GcEffect::NoGc => scoop_identity::GcEffect::NoGc,
    };
    let root_plan = match lir_effect {
        mir::GcEffect::Managed => lir::DependencyExternalCallableRootPlanV1::ManagedStatepoint,
        mir::GcEffect::NoGc => lir::DependencyExternalCallableRootPlanV1::NoGc,
    };
    let canonical_arguments = if has_receiver {
        let storage = scoop_identity::CanonicalScoopStorage::new(
            unit,
            0,
            std::num::NonZeroU64::new(1).unwrap(),
            scoop_identity::ScoopAbiValueShape::Aggregate,
        );
        vec![scoop_identity::ScoopAbiArgument::elided_zst(storage).unwrap()]
    } else {
        Vec::new()
    };
    let canonical = scoop_identity::CanonicalScoopAbiFunctionSignature::new(
        exact,
        canonical_arguments,
        scoop_identity::ScoopAbiReturn::unit_void(),
        canonical_effect,
    )
    .unwrap();
    let selected_record = lir::SelectedDependencyLirCallableV1::new(
        provider,
        declaration,
        target,
        canonical,
        lir::CallingConvention::Cdecl,
        root_plan,
    )
    .unwrap();
    let lir_foundation = lir::OdrFreeLirFoundation::try_new(
        input.module().cone,
        lir::CanonicalLirFoundation::empty(),
    )
    .unwrap();
    let lir_bridge = lir::CrossConeLirBridgeSectionV1::try_new(
        &lir_foundation,
        Vec::new(),
        vec![selected_record],
    )
    .unwrap();
    let selected_lir = lir::SelectedDependencyLirSet::try_from_bridge(&lir_bridge).unwrap();
    (input, selected_lir, target)
}
