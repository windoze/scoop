use super::*;
use la_arena::Arena;
use scoop_identity::{
    CallableMaterialization, CallableMaterializationContext, CallableTemplateOwner,
    CanonicalIdentifier, CborIdentityRecord, DeclarationScope, DefinitionOwnerChain, Effect,
    ExactCallableSignature, GeneratedCallableKey, InitializationUnitKey, PackagePath,
    PersistentGeneratedCallableId, PersistentPropertyId, PropertyOwner, SourceDeclarationSite,
};

pub(super) fn unit_id() -> InitializationUnitId {
    InitializationUnitId::from_raw(0_u32.into())
}
pub(super) fn unit_identity(name: &str) -> InitializationUnitIdentityRecord {
    let property = PersistentPropertyId::from_source_declaration(&SourceDeclarationKey::property(
        SourceDeclarationSite::new(
            ConeIdentity::SINGLE_FILE,
            PackagePath::root(),
            DefinitionOwnerChain::top_level(),
            DeclarationScope::ConeWide,
        )
        .unwrap(),
        CanonicalIdentifier::new(name).unwrap(),
    ))
    .unwrap();
    CborIdentityRecord::from_key(InitializationUnitKey::TopLevelProperty(property)).unwrap()
}
pub(super) fn fixture() -> Module {
    let mut module = base_module();
    let unit = unit_identity("setting");
    let unit_exact = module
        .meta
        .source_exact_types
        .get(&Type::Unit)
        .unwrap()
        .identity_record()
        .id();
    let string_exact = module
        .meta
        .source_exact_types
        .get(&Type::String)
        .unwrap()
        .identity_record()
        .id();
    let cycle_thrower = module.top_level[0];
    let materialization = module
        .meta
        .source_callable_materializations
        .get(cycle_thrower)
        .unwrap()
        .materialization();
    let body = &mut module.functions[cycle_thrower];
    body.gc_effect = GcEffect::Managed;
    let local = body.body.locals.alloc(Local {
        name: "path".into(),
        ty: Type::String,
        mutable: false,
    });
    body.params.push(Param {
        name: "path".into(),
        ty: Type::String,
        local,
    });
    replace_source(
        &mut module,
        SourceCallableMaterialization::new(
            cycle_thrower,
            materialization,
            ExactCallableSignature::new(Effect::Ordinary, None, vec![string_exact], unit_exact),
            None,
        )
        .unwrap(),
    );
    let mut add = |role| {
        let generated =
            PersistentGeneratedCallableId::from_key(&GeneratedCallableKey::Initialization {
                unit: unit.id(),
                role,
            })
            .unwrap();
        let function = module.functions.alloc(Function {
            debug_source: None,
            gc_effect: GcEffect::Managed,
            name: format!("{role:?}"),
            params: vec![],
            return_ty: Type::Unit,
            body: Body::unreachable(Arena::new()),
        });
        let source = SourceCallableMaterialization::new(
            function,
            CallableMaterialization::new(
                CallableTemplateOwner::Generated(generated),
                CallableMaterializationContext::NoSubstitution,
            ),
            ExactCallableSignature::new(Effect::Ordinary, None, vec![], unit_exact),
            None,
        )
        .unwrap();
        replace_source(&mut module, source);
        module.top_level.push(function);
        function
    };
    let initializer = add(InitializationCallableRole::Initializer);
    let ensure = add(InitializationCallableRole::Ensure);
    let InitializationUnitKey::TopLevelProperty(property) = *unit.key() else {
        unreachable!()
    };
    let storage = module.globals.alloc(Global {
        name: "setting".into(),
        storage_owner: StaticStorageOwner::PropertyBacking(PropertyOwner::Property(property)),
        ty: Type::Unit,
        mutable: true,
        storage: GlobalStorage::Managed {
            initial_state: MirStaticInitialState::ZeroedForRuntimeUnit,
        },
    });
    let failure = module.globals.alloc(Global {
        name: "failure".into(),
        storage_owner: StaticStorageOwner::InitializationFailureRoot(unit.id()),
        ty: Type::Any,
        mutable: true,
        storage: GlobalStorage::Managed {
            initial_state: MirStaticInitialState::ZeroedForRuntimeUnit,
        },
    });
    let failure_root = module
        .initialization_failure_roots
        .alloc(InitializationFailureRoot { global: failure });
    module.initialization_units.alloc(InitializationUnit {
        identity: unit,
        display_name: "setting".into(),
        schedule: InitializationSchedule::EagerStartup,
        kind: InitializationUnitKind::EagerTopLevel { storage },
        initializer,
        ensure,
        failure_root,
        dependencies: vec![],
        cycle_thrower: InitializationCycleThrower::Local(cycle_thrower),
    });
    module
}

pub(super) fn replace_source(module: &mut Module, replacement: SourceCallableMaterialization) {
    let mut records: Vec<_> = module
        .meta
        .source_callable_materializations
        .iter()
        .filter(|source| source.function() != replacement.function())
        .cloned()
        .collect();
    records.push(replacement);
    module.meta.source_callable_materializations =
        SourceCallableMaterializations::checked(records).unwrap();
    module.meta.callable_signatures = MirCallableSignatures::checked(
        module
            .meta
            .source_callable_materializations
            .iter()
            .map(|source| source.signature_record().clone())
            .collect(),
    )
    .unwrap();
}

pub(super) fn bind_input(module: Module) -> Result<ConeMirInput, ConeMirInputError> {
    let selected = crate::SelectedExternalMirSet::empty(module.cone);
    let output = crate::DependencyMirOutput::try_new(module, selected).unwrap();
    let module = output.module();
    let foundation = output.strong_foundation().unwrap();
    let production = CoreBootstrapBridgeSectionV1::try_new(
        module.cone,
        EntryMirBridgeBranchV1::Library,
        StrongCallableBridgeSurfaceV1::from_foundation(&foundation),
    )
    .unwrap();
    ConeMirInput::try_new(output, production, Vec::new())
}

fn base_module() -> Module {
    use scoop_identity::{
        CoreBuiltinNominal, PersistentFunctionId, PersistentTypeId, SourceNominalKind,
    };
    let mut functions = Arena::new();
    let entry = functions.alloc(Function {
        debug_source: None,
        gc_effect: GcEffect::NoGc,
        name: "cycle".to_string(),
        params: Vec::new(),
        return_ty: Type::Unit,
        body: Body::unreachable(Arena::new()),
    });
    let mut module = Module {
        release_hooks: Arena::new(),
        cone: ConeIdentity::SINGLE_FILE,
        functions,
        extern_functions: Arena::new(),
        globals: Arena::new(),
        initialization_units: Arena::new(),
        initialization_failure_roots: Arena::new(),
        objects: Arena::new(),
        object_types: Arena::new(),
        singleton_values: Arena::new(),
        singleton_published_roots: Arena::new(),
        callback_bridges: Arena::new(),
        foreign_callback_adapters: Arena::new(),
        foreign_callback_families: Arena::new(),
        foreign_callback_bridges: Arena::new(),
        function_types: Arena::new(),
        closure_classes: Arena::new(),
        closure_invoke_functions: Arena::new(),
        top_level: vec![entry],
        strings: Arena::new(),
        structs: Arena::new(),
        enums: Arena::new(),
        classes: Arena::new(),
        interfaces: Arena::new(),
        option_core: Vec::new(),
        output: MirOutput::Library,
        meta: MirMeta::default(),
    };
    let site = SourceDeclarationSite::new(
        ConeIdentity::SINGLE_FILE,
        PackagePath::root(),
        DefinitionOwnerChain::top_level(),
        DeclarationScope::ConeWide,
    )
    .unwrap();
    let string = PersistentTypeId::from_source_declaration(&SourceDeclarationKey::nominal(
        site,
        CanonicalIdentifier::new("TestString").unwrap(),
        SourceNominalKind::Class,
        0,
    ))
    .unwrap();
    module.meta.source_exact_types = SourceExactTypeIdentities::checked(
        [
            (
                Type::Unit,
                CoreBuiltinNominal::Unit.identity_record().id(),
                ConeIdentity::CORE,
            ),
            (
                Type::Any,
                CoreBuiltinNominal::Any.identity_record().id(),
                ConeIdentity::CORE,
            ),
            (Type::String, string, ConeIdentity::SINGLE_FILE),
        ]
        .into_iter()
        .map(|(ty, nominal, owner)| {
            SourceExactTypeIdentity::checked(
                ty,
                CborIdentityRecord::from_key(ExactTypeKey::Nominal(nominal)).unwrap(),
                SourceExactTypeOrigin::Nominal(owner),
            )
            .unwrap()
        })
        .collect(),
    )
    .unwrap();
    let declaration = SourceDeclarationKey::function(
        SourceDeclarationSite::new(
            ConeIdentity::SINGLE_FILE,
            PackagePath::root(),
            DefinitionOwnerChain::top_level(),
            DeclarationScope::ConeWide,
        )
        .unwrap(),
        CanonicalIdentifier::new("cycle").unwrap(),
        0,
        None,
        vec![],
    );
    replace_source(
        &mut module,
        SourceCallableMaterialization::new(
            entry,
            CallableMaterialization::new(
                CallableTemplateOwner::Function(
                    PersistentFunctionId::from_source_declaration(&declaration).unwrap(),
                ),
                CallableMaterializationContext::NoSubstitution,
            ),
            ExactCallableSignature::new(
                Effect::Ordinary,
                None,
                vec![],
                crate::core_unit_exact_type(),
            ),
            None,
        )
        .unwrap(),
    );
    module
}
