use super::*;
use la_arena::Arena;
use scoop_identity::{
    CallableMaterialization, CallableMaterializationContext, CallableTemplateOwner,
    CanonicalIdentifier, CborIdentityRecord, DeclarationScope, DefinitionOwnerChain,
    ExactCallableSignature, PackagePath, PersistentPropertyId, PropertyOwner,
    SourceDeclarationSite,
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
    let (mut module, _) = crate::validation::tests::module_with_variants(vec![]);
    module.output = MirOutput::Library;
    crate::validation::tests::register_test_exact_type(&mut module, &Type::Any);
    crate::validation::tests::register_test_exact_type(&mut module, &Type::String);
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
    module.validate().unwrap();
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

pub(super) fn seal(
    module: Module,
) -> Result<SingleConeStrongMirInput, SingleConeStrongMirInputError> {
    let selected = crate::SelectedExternalMirSet::empty(module.cone);
    let output = crate::DependencyMirOutput::try_new(module, selected).unwrap();
    let module = output.module();
    let foundation = output.strong_foundation().unwrap();
    let production = CoreBootstrapBridgeSectionV1::try_new(
        module.cone,
        EntryMirBridgeBranchV1::Library,
        StrongCallableBridgeSurfaceV1::from_odr_free_foundation(&foundation),
    )
    .unwrap();
    SingleConeStrongMirInput::try_new(output, foundation, production, Vec::new())
}
