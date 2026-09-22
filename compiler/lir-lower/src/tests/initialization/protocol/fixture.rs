use super::*;

pub(super) struct ImportedInitialization {
    pub(super) input: mir::SingleConeStrongMirInput,
    pub(super) runtime_string: lir::ExternalTypeDescriptor,
    pub(super) callable: lir::SelectedDependencyLirCallableV1,
}

pub(super) fn imported_initialization() -> ImportedInitialization {
    imported_initialization_from(ConeIdentity::CORE)
}

pub(super) fn imported_initialization_from(provider: ConeIdentity) -> ImportedInitialization {
    let mut core_builder = Builder::new();
    let mut core_locals = Arena::new();
    let core_parameter = core_locals.alloc(local("value", mir::Type::String));
    let core_function = core_builder.user_fn_full(
        "__scoopThrowInitializationCycle",
        vec![mir::Param {
            name: "value".to_string(),
            ty: mir::Type::String,
            local: core_parameter,
        }],
        mir::Type::Unit,
        core_locals,
        Vec::new(),
    );
    let mut core_module = core_builder.finish(core_function);
    core_module.cone = provider;
    core_module.output = mir::MirOutput::Library;
    let mir::CallableSignatureSubject::Strong(implementation) = core_module
        .meta
        .callable_signature_subject(core_function)
        .unwrap()
    else {
        panic!("test core callable must have strong ownership")
    };
    let scoop_identity::CallableOwner::Function(definition) = implementation else {
        panic!("test core callable must be a source function")
    };
    let core_foundation = mir::OdrFreeMirFoundation::from_module(&core_module).unwrap();
    let strong = mir::StrongCallableBridgeSurfaceV1::from_odr_free_foundation(&core_foundation);
    let exact = strong
        .bridges()
        .iter()
        .find(|bridge| bridge.implementation() == implementation)
        .unwrap()
        .signature()
        .clone();
    let core_production = mir::CoreBootstrapBridgeSectionV1::try_new(
        provider,
        mir::EntryMirBridgeBranchV1::Library,
        strong.with_initialization_cycle(definition).unwrap(),
    )
    .unwrap();
    let core_input = mir::SingleConeStrongMirInput::try_new(
        core_module,
        core_foundation.clone(),
        core_production,
        Vec::new(),
        mir::StrongExternalCallableInput::Unused,
    )
    .unwrap();
    let core_lir = crate::lower(
        &core_input,
        crate::RuntimeStringDescriptor::Local,
        &lir::SelectedExternalLirSet::empty(provider),
        lir::LirTargetProfile::DARWIN_AARCH64,
    )
    .unwrap();

    let mut pending = PendingIdentityValidation::new();
    pending.register_authority(provider).unwrap();
    let identities = pending.finish().unwrap();
    let mut session = SemanticIdentitySession::new();
    let (_, imported_mir_identities, _) = session
        .import(
            provider,
            SemanticOriginFingerprint::new([1; 32], [2; 32], [3; 32]),
            &identities,
        )
        .unwrap()
        .into_parts();
    let imported_mir =
        mir::ImportedMirFoundation::from_odr_free(core_foundation, imported_mir_identities);
    let mir_callable = imported_mir
        .project_initialization_cycle_thrower(core_input.production(), definition, exact.clone())
        .unwrap();
    let selected_mir = mir::SelectedExternalMirSet::empty(ConeIdentity::SINGLE_FILE)
        .with_initialization_cycle(mir_callable)
        .unwrap();
    let mir_selection = selected_mir.initialization_cycle().unwrap();
    let mut ordinary_builder = Builder::new();
    let mut caller_locals = Arena::new();
    let caller_argument = caller_locals.alloc(local("message", mir::Type::String));
    let caller = ordinary_builder.user_fn_full(
        "caller",
        vec![param("message", mir::Type::String, caller_argument)],
        mir::Type::Unit,
        caller_locals,
        Vec::new(),
    );
    let mut ordinary_module = ordinary_builder.finish(caller);
    ordinary_module.output = mir::MirOutput::Library;
    let imported_use = ordinary_module.meta.external_callables.alloc(
        selected_mir
            .callable_use(mir_selection, mir::GcEffect::Managed)
            .expect("selected MIR callable mints one use"),
    );
    let entry = ordinary_module.functions[caller].body.entry;
    ordinary_module.functions[caller].body.blocks[entry]
        .statements
        .push(call_stmt(mir::Call {
            target: mir::CallTarget {
                kind: mir::CallKind::Direct,
                callee: mir::Callee::External(imported_use),
            },
            args: vec![local_expr(caller_argument, mir::Type::String)],
            pending: mir::CoroutinePendingContext::Root,
        }));
    let ordinary_foundation = mir::OdrFreeMirFoundation::from_module(&ordinary_module).unwrap();
    let ordinary_production = mir::CoreBootstrapBridgeSectionV1::try_new(
        ConeIdentity::SINGLE_FILE,
        mir::EntryMirBridgeBranchV1::Library,
        mir::StrongCallableBridgeSurfaceV1::from_odr_free_foundation(&ordinary_foundation),
    )
    .unwrap();
    let ordinary_input = mir::SingleConeStrongMirInput::try_new(
        ordinary_module,
        ordinary_foundation,
        ordinary_production,
        Vec::new(),
        mir::StrongExternalCallableInput::Selected(&selected_mir),
    )
    .unwrap();

    let canonical_lir = core_lir.foundation().as_canonical().clone();
    let decoded_lir = scoop_wire::decode_canonical::<lir::DecodedLirFoundation>(
        &scoop_wire::encode(&canonical_lir).unwrap(),
        scoop_wire::DecodeLimits::default(),
    )
    .unwrap();
    let mut pending = PendingIdentityValidation::new();
    pending.register_authority(provider).unwrap();
    pending.register_authority(definition).unwrap();
    let decoded_exact_types = core_input
        .module()
        .meta
        .source_exact_types
        .iter()
        .map(|identity| {
            scoop_wire::decode_canonical::<
                scoop_identity::DecodedCborIdentityRecord<
                    scoop_identity::PersistentExactTypeId,
                    scoop_identity::DecodedExactTypeKey,
                >,
            >(
                &scoop_wire::encode(identity.identity_record()).unwrap(),
                scoop_wire::DecodeLimits::default(),
            )
            .unwrap()
        })
        .collect::<Vec<_>>();
    for identity in core_input.module().meta.source_exact_types.iter() {
        match identity.identity_record().key() {
            scoop_identity::ExactTypeKey::Nominal(owner) => {
                pending.register_authority(*owner).unwrap();
            }
            scoop_identity::ExactTypeKey::NominalApplication { origin, .. } => {
                pending.register_authority(*origin).unwrap();
            }
            scoop_identity::ExactTypeKey::Tuple(_)
            | scoop_identity::ExactTypeKey::Function { .. }
            | scoop_identity::ExactTypeKey::RawPointer(_)
            | scoop_identity::ExactTypeKey::NativeFunctionPointer { .. } => {}
        }
    }
    for identity in &decoded_exact_types {
        pending
            .register(scoop_identity::IdentityLayer::Hir, identity)
            .unwrap();
    }
    decoded_lir.register_identities(&mut pending).unwrap();
    for identity in &decoded_exact_types {
        pending.resolve(identity).unwrap();
    }
    decoded_lir.resolve_identities(&mut pending).unwrap();
    let identities = pending.finish().unwrap();
    let mut session = SemanticIdentitySession::new();
    let (_, _, imported_lir_identities) = session
        .import(
            provider,
            SemanticOriginFingerprint::new([4; 32], [5; 32], [6; 32]),
            &identities,
        )
        .unwrap()
        .into_parts();
    let imported_lir = lir::ImportedLirFoundation::from_odr_free(
        core_lir.foundation().clone(),
        imported_lir_identities,
    );
    let definitions =
        lir::StrongObjectSymbolSurfaceV1::from_odr_free_foundation(core_lir.foundation()).unwrap();
    let core_bridge = core_lir.initialization_cycle_abi().unwrap();
    let lir_callable = imported_lir
        .project_initialization_cycle_thrower(
            core_bridge,
            &definitions,
            scoop_identity::StrongCallableDefinitionOwner::Function(definition),
            exact,
        )
        .unwrap();
    let string_exact = core_input
        .module()
        .meta
        .source_exact_types
        .iter()
        .find(|identity| identity.ty() == &mir::Type::String)
        .unwrap()
        .identity_record()
        .id();
    let runtime_string = imported_lir
        .project_type_descriptor(&definitions, string_exact)
        .unwrap();
    ImportedInitialization {
        input: ordinary_input,
        runtime_string,
        callable: lir_callable,
    }
}
