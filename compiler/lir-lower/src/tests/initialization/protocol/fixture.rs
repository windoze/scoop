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
    let mut core_module = core_builder.finish_with_types(core_function, provider, vec![]);
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
        &[],
        &lir::SelectedExternalLirSet::empty(provider),
        lir::LirTargetProfile::DARWIN_AARCH64,
    )
    .unwrap();

    let target = scoop_identity::StrongCallableDefinitionOwner::Function(definition);
    let declaration = scoop_identity::DependencyCallableDeclarationId::Function(definition);
    let export = mir::ParamFreeMirCallableExportV1::try_new(
        declaration,
        target,
        exact.clone(),
        scoop_mir::GcEffect::Managed,
    )
    .unwrap();
    let bridge = mir::CrossConeMirBridgeSectionV1::try_new(
        provider,
        core_input.foundation(),
        vec![export],
        Vec::new(),
    )
    .unwrap();
    let lir_bridge =
        crate::lower_cross_cone_bridge_section(&core_input, &bridge, &core_lir).unwrap();
    let lir_callable = lir::SelectedDependencyLirCallableV1::from_export(
        provider,
        lir_bridge.exports()[0].clone(),
    );
    let mir_callable =
        mir::SelectedDependencyMirCallableV1::try_new(provider, declaration, target, exact)
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
    let imported_string = core_input
        .module()
        .meta
        .source_exact_types
        .get(&mir::Type::String)
        .unwrap()
        .clone();
    let mut ordinary_module = ordinary_builder.finish_with_types(
        caller,
        ConeIdentity::SINGLE_FILE,
        vec![imported_string],
    );
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

    let string_exact = core_input
        .module()
        .meta
        .source_exact_types
        .iter()
        .find(|identity| identity.ty() == &mir::Type::String)
        .unwrap()
        .identity_record()
        .id();
    let runtime_string =
        lir::ExternalTypeDescriptor::new(core_input.module().cone, string_exact).unwrap();
    ImportedInitialization {
        input: ordinary_input,
        runtime_string,
        callable: lir_callable,
    }
}
