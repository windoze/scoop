use super::*;
use scoop_identity::{
    GeneratedCallableKey, InitializationCallableRole, PersistentGeneratedCallableId,
};
use scoop_lir::{CallableBodyIdentity, LayoutIdentity, MaterializationRoot, StaticStorageIdentity};

pub(super) fn module(target: TargetProfileId) -> Module {
    let mut module = for_target(values_module(), target);
    let string = module.globals.iter().next().unwrap().0;
    add_storage(
        &mut module,
        "encodedInteger",
        LirType::I64,
        RefScan::None,
        LirStaticInitialState::EncodedStaticValue {
            payload: LirConstantImage::Integer(scoop_lir::LirIntegerConstant::Signed64(42)),
        },
    );
    add_storage(
        &mut module,
        "encodedString",
        MANAGED_PTR,
        RefScan::References(vec![0]),
        LirStaticInitialState::EncodedStaticValue {
            payload: LirConstantImage::GlobalPointer {
                global: string,
                kind: PointerKind::Managed,
            },
        },
    );
    let empty = module_empty_struct(&mut module);
    add_storage(
        &mut module,
        "emptyStorage",
        LirType::Struct(empty),
        RefScan::None,
        LirStaticInitialState::EncodedStaticValue {
            payload: LirConstantImage::Struct {
                struct_id: empty,
                fields: Vec::new(),
            },
        },
    );
    add_initialization(&mut module, "initFirst");
    add_initialization(&mut module, "initSecond");
    module
}

pub(super) fn encoded_only(target: TargetProfileId, initial: u64) -> Module {
    let mut module = for_target(values_module(), target);
    module.globals = Arena::default();
    module.functions.clear();
    add_storage(
        &mut module,
        "encodedOnly",
        LirType::I64,
        RefScan::None,
        LirStaticInitialState::EncodedStaticValue {
            payload: LirConstantImage::Integer(scoop_lir::LirIntegerConstant::Signed64(initial)),
        },
    );
    module
}

pub(super) fn zeroed_odr_only(target: TargetProfileId) -> Module {
    let mut module = for_target(values_module(), target);
    module.globals = Arena::default();
    module.functions.clear();
    module.globals.alloc(storage(
        module.meta.target_profile,
        "zeroedOdr",
        odr_static_storage_identity("zeroedOdr"),
        LirType::I64,
        RefScan::None,
        LirStaticInitialState::ZeroedForRuntimeUnit,
    ));
    module
}

fn module_empty_struct(module: &mut Module) -> scoop_lir::StructDefId {
    module.structs.alloc_scoop(
        test_physical_exact("EmptyStatic", scoop_identity::SourceNominalKind::Struct),
        "EmptyStatic".to_owned(),
        0,
        1,
        false,
        Vec::new(),
    )
}

fn add_storage(
    module: &mut Module,
    name: &str,
    ty: LirType,
    scan: RefScan,
    initial_state: LirStaticInitialState,
) -> scoop_lir::GlobalId {
    module.globals.alloc(storage(
        module.meta.target_profile,
        name,
        static_storage_identity(name),
        ty,
        scan,
        initial_state,
    ))
}

fn storage(
    target: scoop_lir::LirTargetProfile,
    name: &str,
    identity: StaticStorageIdentity,
    ty: LirType,
    scan: RefScan,
    initial_state: LirStaticInitialState,
) -> Global {
    Global {
        address_kind: PointerKind::Raw,
        scan,
        init: GlobalInit::Storage {
            identity,
            layout: LayoutIdentity::managed_value(
                test_exact_type(name),
                target,
                MaterializationRoot::cone_owned(),
            )
            .unwrap()
            .into(),
            ty,
            initial_state,
        },
    }
}

fn add_initialization(module: &mut Module, name: &str) {
    let owner =
        scoop_identity::PersistentTypeId::from_source_declaration(&SourceDeclarationKey::nominal(
            SourceDeclarationSite::new(
                ConeIdentity::SINGLE_FILE,
                PackagePath::root(),
                DefinitionOwnerChain::top_level(),
                DeclarationScope::ConeWide,
            )
            .unwrap(),
            CanonicalIdentifier::new(name).unwrap(),
            scoop_identity::SourceNominalKind::Object,
            0,
        ))
        .unwrap();
    let identity = CborIdentityRecord::from_key(InitializationUnitKey::Object(owner)).unwrap();
    let id = identity.id();
    let value = module.globals.alloc(storage(
        module.meta.target_profile,
        name,
        StaticStorageIdentity::singleton_published_root(owner, MaterializationRoot::cone_owned())
            .unwrap(),
        MANAGED_PTR,
        RefScan::References(vec![0]),
        LirStaticInitialState::ZeroedForRuntimeUnit,
    ));
    let failure = module.globals.alloc(storage(
        module.meta.target_profile,
        &format!("{name}Failure"),
        StaticStorageIdentity::initialization_failure_root(id, MaterializationRoot::cone_owned())
            .unwrap(),
        MANAGED_PTR,
        RefScan::References(vec![0]),
        LirStaticInitialState::ZeroedForRuntimeUnit,
    ));
    let first = module.functions.len();
    for role in [
        InitializationCallableRole::Initializer,
        InitializationCallableRole::Ensure,
    ] {
        let generated =
            PersistentGeneratedCallableId::from_key(&GeneratedCallableKey::Initialization {
                unit: id,
                role,
            })
            .unwrap();
        module.functions.push(empty_function(
            CallableBodyIdentity::for_generated_callable(generated).unwrap(),
        ));
    }
    module
        .initialization_units
        .alloc(scoop_lir::InitializationUnit {
            identity,
            display_name: format!("object:{name}"),
            schedule: scoop_lir::InitializationSchedule::LazyAccess,
            kind: scoop_lir::InitializationUnitKind::LazySingleton {
                published_root: value,
            },
            failure_root: failure,
            initializer: managed_local_function_ref(first),
            ensure: managed_local_function_ref(first + 1),
            dependencies: Vec::new(),
        });
}

fn empty_function(callable_body: CallableBodyIdentity) -> Function {
    let mut blocks = Arena::new();
    let entry = blocks.alloc(BasicBlock {
        name: "entry".to_owned(),
        instructions: Vec::new(),
        terminator: Terminator::Return { value: None },
    });
    Function {
        callable_body,
        gc_effect: GcEffect::Managed,
        signature: scoop_lir::ScoopAbiSignature::new(
            Vec::new(),
            scoop_lir::AbiReturn::UnitVoid,
            scoop_lir::CallingConvention::Cdecl,
        ),
        call_targets: CallTargets::default(),
        safepoints: Default::default(),
        locals: Arena::new(),
        temps: Arena::new(),
        blocks,
        entry,
    }
}
