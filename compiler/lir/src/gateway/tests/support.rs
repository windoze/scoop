use super::*;
use scoop_identity::{
    CanonicalIdentifier, CborIdentityRecord, CoreBuiltinNominal, DeclarationScope,
    DefinitionOwnerChain, ExactOrdinaryNoArgUnitSignature, ExactTypeKey,
    ExecutableSourceEntryIdentity, GeneratedCallableKey, InitializationCallableRole,
    InitializationUnitKey, PackagePath, PersistentExactTypeId, PersistentGeneratedCallableId,
    PersistentPropertyId, PropertyOwner, SourceDeclarationKey, SourceDeclarationSite,
};

fn site() -> SourceDeclarationSite {
    SourceDeclarationSite::new(
        ConeIdentity::SINGLE_FILE,
        PackagePath::root(),
        DefinitionOwnerChain::top_level(),
        DeclarationScope::ConeWide,
    )
    .unwrap()
}
fn global(identity: StaticStorageIdentity) -> Global {
    Global {
        address_kind: PointerKind::Raw,
        scan: RefScan::References(vec![0]),
        init: GlobalInit::Storage {
            identity,
            layout: LayoutIdentity::managed_value(
                crate::tests::test_physical_exact(
                    "failure",
                    scoop_identity::SourceNominalKind::Class,
                ),
                LirTargetProfile::DARWIN_AARCH64,
                MaterializationRoot::cone_owned(),
            )
            .unwrap()
            .into(),
            ty: MANAGED_PTR,
            initial_state: LirStaticInitialState::ZeroedForRuntimeUnit,
        },
    }
}
pub(super) fn function(body: CallableBodyIdentity, effect: GcEffect) -> Function {
    let mut blocks = Arena::new();
    let entry = blocks.alloc(BasicBlock {
        name: "entry".into(),
        instructions: vec![],
        terminator: Terminator::Return { value: None },
    });
    Function {
        callable_body: body,
        gc_effect: effect,
        signature: ScoopAbiSignature::new(vec![], AbiReturn::UnitVoid, CallingConvention::Cdecl),
        call_targets: CallTargets::default(),
        safepoints: SafepointIdentities::default(),
        locals: Arena::new(),
        temps: Arena::new(),
        blocks,
        entry,
    }
}
pub(super) fn root(managed: bool) -> Module {
    let declaration = CborIdentityRecord::from_key(SourceDeclarationKey::function(
        site(),
        CanonicalIdentifier::new("main").unwrap(),
        0,
        None,
        vec![],
    ))
    .unwrap();
    let unit = PersistentExactTypeId::from_key(&ExactTypeKey::Nominal(
        CoreBuiltinNominal::Unit.identity_record().id(),
    ))
    .unwrap();
    let source = ExecutableSourceEntryIdentity::try_new(
        &declaration,
        ExactOrdinaryNoArgUnitSignature::new(unit),
    )
    .unwrap();
    let mut locals = LocalFunctionIdentities::default();
    let (entry, effect) = if managed {
        (
            LocalFunctionRef::Managed(locals.alloc_managed()),
            GcEffect::Managed,
        )
    } else {
        (LocalFunctionRef::NoGc(locals.alloc_no_gc()), GcEffect::NoGc)
    };
    locals.alloc_managed();
    let mut module = crate::test_support::module(vec![
        function(
            CallableBodyIdentity::for_function(declaration.id()).unwrap(),
            effect,
        ),
        function(crate::tests::callable_body("other"), GcEffect::Managed),
    ]);
    module.output = LirOutput::Executable { entry };
    let failure = module.globals.alloc(global(
        StaticStorageIdentity::root_entry_failure_root(
            module.cone,
            source.main(),
            MaterializationRoot::cone_owned(),
        )
        .unwrap(),
    ));
    let body = CallableBodyIdentity::for_root_gateway(module.cone, source.main()).unwrap();
    module
        .functions
        .push(super::body::gateway(body, entry, Some(failure)));
    module
}
pub(super) fn eager() -> Module {
    let property = PersistentPropertyId::from_source_declaration(&SourceDeclarationKey::property(
        site(),
        CanonicalIdentifier::new("value").unwrap(),
    ))
    .unwrap();
    let identity =
        CborIdentityRecord::from_key(InitializationUnitKey::TopLevelProperty(property)).unwrap();
    let id = identity.id();
    let mut locals = LocalFunctionIdentities::default();
    let ensure = locals.alloc_managed();
    let initializer = locals.alloc_managed();
    let body = |role| {
        CallableBodyIdentity::for_generated_callable(
            PersistentGeneratedCallableId::from_key(&GeneratedCallableKey::Initialization {
                unit: id,
                role,
            })
            .unwrap(),
        )
        .unwrap()
    };
    let mut module = crate::test_support::module(vec![
        function(body(InitializationCallableRole::Ensure), GcEffect::Managed),
        function(
            body(InitializationCallableRole::Initializer),
            GcEffect::Managed,
        ),
    ]);
    let failure_root = module.globals.alloc(global(
        StaticStorageIdentity::initialization_failure_root(id, MaterializationRoot::cone_owned())
            .unwrap(),
    ));
    let storage = module.globals.alloc(global(
        StaticStorageIdentity::property_backing(
            PropertyOwner::Property(property),
            MaterializationRoot::cone_owned(),
        )
        .unwrap(),
    ));
    module.initialization_units.alloc(InitializationUnit {
        identity,
        display_name: "value".into(),
        schedule: InitializationSchedule::EagerStartup,
        kind: InitializationUnitKind::EagerTopLevel { storage },
        failure_root,
        initializer,
        ensure,
        dependencies: vec![],
    });
    module.functions.push(super::body::gateway(
        CallableBodyIdentity::for_initialization_startup_gateway(id).unwrap(),
        LocalFunctionRef::Managed(ensure),
        None,
    ));
    module
}
