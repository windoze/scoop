use la_arena::Arena;
use scoop_identity::*;

use super::TARGET;
use crate::*;

pub(super) fn attach_eager_initialization(
    module: &mut Module,
    name: &str,
    storage_exact: PersistentExactTypeId,
) -> PersistentInitializationUnitId {
    let property = PersistentPropertyId::from_source_declaration(&SourceDeclarationKey::property(
        SourceDeclarationSite::new(
            module.cone,
            PackagePath::root(),
            DefinitionOwnerChain::top_level(),
            DeclarationScope::ConeWide,
        )
        .unwrap(),
        CanonicalIdentifier::new(name).unwrap(),
    ))
    .unwrap();
    let identity =
        CborIdentityRecord::from_key(InitializationUnitKey::TopLevelProperty(property)).unwrap();
    let unit = identity.id();
    let storage = module.globals.alloc(storage_global(
        StaticStorageIdentity::property_backing(
            PropertyOwner::Property(property),
            MaterializationRoot::cone_owned(),
        )
        .unwrap(),
        storage_exact,
    ));
    let failure_root = module.globals.alloc(storage_global(
        StaticStorageIdentity::initialization_failure_root(unit, MaterializationRoot::cone_owned())
            .unwrap(),
        storage_exact,
    ));

    let mut local_functions = LocalFunctionIdentities::default();
    for _ in &module.functions {
        local_functions.alloc_managed();
    }
    let initializer = local_functions.alloc_managed();
    let ensure = local_functions.alloc_managed();
    local_functions.alloc_managed();
    module.functions.extend([
        initialization_function(unit, InitializationCallableRole::Initializer),
        initialization_function(unit, InitializationCallableRole::Ensure),
        empty_initialization_function(
            CallableBodyIdentity::for_initialization_startup_gateway(unit).unwrap(),
        ),
    ]);
    module.initialization_units.alloc(InitializationUnit {
        identity,
        display_name: format!("top-level:{name}"),
        schedule: InitializationSchedule::EagerStartup,
        kind: InitializationUnitKind::EagerTopLevel { storage },
        failure_root,
        initializer,
        ensure,
        dependencies: Vec::new(),
    });
    unit
}

fn initialization_function(
    unit: PersistentInitializationUnitId,
    role: InitializationCallableRole,
) -> Function {
    let generated =
        PersistentGeneratedCallableId::from_key(&GeneratedCallableKey::Initialization {
            unit,
            role,
        })
        .unwrap();
    empty_initialization_function(CallableBodyIdentity::for_generated_callable(generated).unwrap())
}

fn empty_initialization_function(callable_body: CallableBodyIdentity) -> Function {
    let mut blocks = Arena::new();
    let entry = blocks.alloc(BasicBlock {
        name: "entry".to_owned(),
        instructions: Vec::new(),
        terminator: Terminator::Return { value: None },
    });
    Function {
        callable_body,
        gc_effect: crate::GcEffect::Managed,
        signature: ScoopAbiSignature::new(
            Vec::new(),
            AbiReturn::UnitVoid,
            crate::CallingConvention::Cdecl,
        ),
        call_targets: CallTargets::default(),
        safepoints: SafepointIdentities::default(),
        locals: Arena::new(),
        temps: Arena::new(),
        blocks,
        entry,
    }
}

fn storage_global(identity: StaticStorageIdentity, exact: PersistentExactTypeId) -> Global {
    Global {
        address_kind: PointerKind::Raw,
        scan: RefScan::References(vec![0]),
        init: GlobalInit::Storage {
            identity,
            layout: LayoutIdentity::managed_value(exact, TARGET, MaterializationRoot::cone_owned())
                .unwrap()
                .into(),
            ty: MANAGED_PTR,
            initial_state: LirStaticInitialState::ZeroedForRuntimeUnit,
            thread_local: false,
        },
    }
}
