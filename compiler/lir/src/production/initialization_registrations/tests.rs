use la_arena::Arena;
use scoop_identity::{
    CanonicalIdentifier, CborIdentityRecord, ConeIdentity, DeclarationScope, DefinitionOwnerChain,
    ExactTypeKey, GeneratedCallableKey, InitializationCallableRole, InitializationUnitKey,
    PackagePath, PersistentExactTypeId, PersistentGeneratedCallableId, PersistentPropertyId,
    PersistentTypeId, PropertyOwner, SourceDeclarationKey, SourceDeclarationSite,
    SourceNominalKind,
};

use super::*;
use crate::{
    AbiReturn, BasicBlock, CallTargets, CallableBodyIdentity, CallingConvention, Function, Global,
    LayoutIdentity, LirStaticInitialState, LirType, LocalFunctionIdentities, MaterializationRoot,
    PointerKind, SafepointIdentities, ScoopAbiSignature, StaticStorageIdentity, StructDefs,
    Terminator,
};

#[test]
fn closes_eager_unit_storage_callables_gateway_and_dependencies() {
    let mut fixture = Fixture::eager();
    let dependency = fixture.add_eager_dependency("dependency");
    fixture.units[fixture.unit].dependencies.push(dependency);

    let plans = fixture.build().unwrap();

    assert_eq!(plans.producer(), ConeIdentity::SINGLE_FILE);
    assert_eq!(plans.units().len(), 2);
    assert!(
        plans
            .units()
            .windows(2)
            .all(|pair| pair[0].unit() < pair[1].unit())
    );
    let plan = plans
        .units()
        .iter()
        .find(|plan| plan.unit() == fixture.unit_id)
        .unwrap();
    assert_eq!(plan.diagnostic_path(), "top-level:value");
    assert_eq!(plan.initializer(), fixture.initializer);
    assert_eq!(plan.ensure(), fixture.ensure);
    assert_eq!(plan.dependencies().len(), 1);
    assert_eq!(
        plan.schedule().gateway(),
        Some(startup_gateway_body(fixture.unit_id).unwrap())
    );
    assert_ne!(plan.storage(), plan.failure_root());
}

#[test]
fn closes_lazy_unit_without_a_startup_gateway() {
    let fixture = Fixture::lazy();

    let plans = fixture.build().unwrap();
    let plan = &plans.units()[0];

    assert_eq!(plan.unit(), fixture.unit_id);
    assert_eq!(
        plan.schedule(),
        StrongInitializationSchedulePlanV1::LazyAccess
    );
    assert_eq!(plan.schedule().gateway(), None);
}

#[test]
fn rejects_non_owner_bound_failure_root_and_nonzeroed_value_storage() {
    let mut wrong_failure = Fixture::eager();
    let other = unit_identity(InitializationUnitKey::TopLevelProperty(property("other")));
    replace_storage_identity(
        &mut wrong_failure.globals[wrong_failure.failure],
        StaticStorageIdentity::initialization_failure_root(
            other.id(),
            MaterializationRoot::cone_owned(),
        )
        .unwrap(),
    );
    assert_eq!(
        wrong_failure.build(),
        Err(StrongInitializationUnitSemanticPlanBuildError::FailureRootKey(wrong_failure.unit_id))
    );

    let mut encoded = Fixture::eager();
    let GlobalInit::Storage { initial_state, .. } = &mut encoded.globals[encoded.storage].init
    else {
        unreachable!()
    };
    *initial_state = LirStaticInitialState::EncodedStaticValue {
        payload: crate::LirConstantImage::Integer(crate::LirIntegerConstant::Signed64(0)),
    };
    assert!(matches!(
        encoded.build(),
        Err(
            StrongInitializationUnitSemanticPlanBuildError::NonZeroedStorage {
                role: InitializationStorageRoleV1::Value,
                ..
            }
        )
    ));
}

#[test]
fn rejects_a_static_place_token_for_nonzero_value_storage() {
    let mut fixture = Fixture::eager();
    replace_storage_identity(
        &mut fixture.globals[fixture.storage],
        StaticStorageIdentity::static_place_for_property(
            PropertyOwner::Property(property("value")),
            MaterializationRoot::cone_owned(),
        )
        .unwrap(),
    );
    assert_eq!(
        fixture.build(),
        Err(StrongInitializationUnitSemanticPlanBuildError::ValueStorageKey(fixture.unit_id)),
    );
}

#[test]
fn rejects_missing_gateway_wrong_callable_and_invalid_dependencies() {
    let mut missing_gateway = Fixture::eager();
    missing_gateway.functions.pop();
    assert!(matches!(
        missing_gateway.build(),
        Err(StrongInitializationUnitSemanticPlanBuildError::StartupGatewaySet { actual: 0, .. })
    ));

    let mut unexpected_gateway = Fixture::lazy();
    unexpected_gateway.functions.push(function(
        CallableBodyIdentity::for_initialization_startup_gateway(unexpected_gateway.unit_id)
            .unwrap(),
    ));
    assert!(matches!(
        unexpected_gateway.build(),
        Err(StrongInitializationUnitSemanticPlanBuildError::UnexpectedStartupGateway { .. })
    ));

    let mut wrong_callable = Fixture::eager();
    wrong_callable.functions[0].callable_body = CallableBodyIdentity::for_function(
        scoop_identity::PersistentFunctionId::from_source_declaration(
            &SourceDeclarationKey::function(
                source_site(),
                CanonicalIdentifier::new("wrong").unwrap(),
                0,
                None,
                Vec::new(),
            ),
        )
        .unwrap(),
    )
    .unwrap();
    assert!(matches!(
        wrong_callable.build(),
        Err(StrongInitializationUnitSemanticPlanBuildError::FunctionReferenceIdentity { .. })
    ));

    let mut duplicate_callable = Fixture::eager();
    duplicate_callable.functions.push(function(
        duplicate_callable.functions[0].callable_body.clone(),
    ));
    assert!(matches!(
        duplicate_callable.build(),
        Err(StrongInitializationUnitSemanticPlanBuildError::FunctionBodySet { actual: 2, .. })
    ));

    let mut self_dependency = Fixture::eager();
    self_dependency.units[self_dependency.unit]
        .dependencies
        .push(self_dependency.unit);
    assert_eq!(
        self_dependency.build(),
        Err(
            StrongInitializationUnitSemanticPlanBuildError::SelfDependency(self_dependency.unit_id)
        )
    );
}

struct Fixture {
    globals: Arena<Global>,
    units: Arena<InitializationUnit>,
    functions: Vec<Function>,
    local_functions: LocalFunctionIdentities,
    unit: InitializationUnitId,
    unit_id: PersistentInitializationUnitId,
    storage: crate::GlobalId,
    failure: crate::GlobalId,
    initializer: PersistentCallableBodyId,
    ensure: PersistentCallableBodyId,
}

impl Fixture {
    fn eager() -> Self {
        let key = InitializationUnitKey::TopLevelProperty(property("value"));
        Self::new(key, true)
    }

    fn lazy() -> Self {
        let owner = nominal("Singleton");
        Self::new(InitializationUnitKey::Object(owner), false)
    }

    fn new(key: InitializationUnitKey, eager: bool) -> Self {
        let identity = unit_identity(key.clone());
        let unit_id = identity.id();
        let storage_identity = match &key {
            InitializationUnitKey::TopLevelProperty(property) => {
                StaticStorageIdentity::property_backing(
                    PropertyOwner::Property(*property),
                    MaterializationRoot::cone_owned(),
                )
                .unwrap()
            }
            InitializationUnitKey::Object(owner) => {
                StaticStorageIdentity::singleton_published_root(
                    *owner,
                    MaterializationRoot::cone_owned(),
                )
                .unwrap()
            }
            _ => unreachable!(),
        };
        let mut globals = Arena::new();
        let storage = globals.alloc(storage_global(
            storage_identity,
            if eager {
                LirType::I64
            } else {
                crate::MANAGED_PTR
            },
            if eager {
                RefScan::None
            } else {
                RefScan::References(vec![0])
            },
            "value",
        ));
        let failure = globals.alloc(storage_global(
            StaticStorageIdentity::initialization_failure_root(
                unit_id,
                MaterializationRoot::cone_owned(),
            )
            .unwrap(),
            crate::MANAGED_PTR,
            RefScan::References(vec![0]),
            "failure",
        ));
        let initializer = generated_body_identity(unit_id, InitializationCallableRole::Initializer);
        let ensure = generated_body_identity(unit_id, InitializationCallableRole::Ensure);
        let gateway = CallableBodyIdentity::for_initialization_startup_gateway(unit_id).unwrap();
        let mut local_functions = LocalFunctionIdentities::default();
        let initializer_ref = local_functions.alloc_managed();
        let ensure_ref = local_functions.alloc_managed();
        let mut functions = vec![function(initializer.clone()), function(ensure.clone())];
        if eager {
            local_functions.alloc_managed();
            functions.push(function(gateway));
        }
        let mut units = Arena::new();
        let unit = units.alloc(InitializationUnit {
            identity,
            display_name: if eager {
                "top-level:value".to_string()
            } else {
                "object:Singleton".to_string()
            },
            schedule: if eager {
                InitializationSchedule::EagerStartup
            } else {
                InitializationSchedule::LazyAccess
            },
            kind: if eager {
                InitializationUnitKind::EagerTopLevel { storage }
            } else {
                InitializationUnitKind::LazySingleton {
                    published_root: storage,
                }
            },
            failure_root: failure,
            initializer: initializer_ref,
            ensure: ensure_ref,
            dependencies: Vec::new(),
        });
        Self {
            globals,
            units,
            functions,
            local_functions,
            unit,
            unit_id,
            storage,
            failure,
            initializer: initializer.id(),
            ensure: ensure.id(),
        }
    }

    fn add_eager_dependency(&mut self, name: &str) -> InitializationUnitId {
        let identity = unit_identity(InitializationUnitKey::TopLevelProperty(property(name)));
        let unit_id = identity.id();
        let storage = self.globals.alloc(storage_global(
            StaticStorageIdentity::property_backing(
                PropertyOwner::Property(property(name)),
                MaterializationRoot::cone_owned(),
            )
            .unwrap(),
            LirType::I64,
            RefScan::None,
            name,
        ));
        let failure = self.globals.alloc(storage_global(
            StaticStorageIdentity::initialization_failure_root(
                unit_id,
                MaterializationRoot::cone_owned(),
            )
            .unwrap(),
            crate::MANAGED_PTR,
            RefScan::References(vec![0]),
            &format!("{name}Failure"),
        ));
        let initializer = generated_body_identity(unit_id, InitializationCallableRole::Initializer);
        let ensure = generated_body_identity(unit_id, InitializationCallableRole::Ensure);
        let initializer_ref = self.local_functions.alloc_managed();
        let ensure_ref = self.local_functions.alloc_managed();
        self.functions.push(function(initializer));
        self.functions.push(function(ensure));
        self.local_functions.alloc_managed();
        self.functions.push(function(
            CallableBodyIdentity::for_initialization_startup_gateway(unit_id).unwrap(),
        ));
        self.units.alloc(InitializationUnit {
            identity,
            display_name: format!("top-level:{name}"),
            schedule: InitializationSchedule::EagerStartup,
            kind: InitializationUnitKind::EagerTopLevel { storage },
            failure_root: failure,
            initializer: initializer_ref,
            ensure: ensure_ref,
            dependencies: Vec::new(),
        })
    }

    fn build(
        &self,
    ) -> Result<
        StrongInitializationUnitSemanticPlanSetV1,
        StrongInitializationUnitSemanticPlanBuildError,
    > {
        let storages = StrongStaticStorageSemanticPlanSetV1::from_parts(
            ConeIdentity::SINGLE_FILE,
            LirTargetProfile::DARWIN_AARCH64,
            &self.globals,
            &StructDefs::default(),
            &crate::EnumDefs::default(),
        )
        .unwrap();
        StrongInitializationUnitSemanticPlanSetV1::from_parts(
            ConeIdentity::SINGLE_FILE,
            LirTargetProfile::DARWIN_AARCH64,
            &self.globals,
            &self.units,
            &self.functions,
            storages,
        )
    }
}

fn function(callable_body: CallableBodyIdentity) -> Function {
    let mut blocks = Arena::new();
    let entry = blocks.alloc(BasicBlock {
        name: "entry".to_string(),
        instructions: Vec::new(),
        terminator: Terminator::Return { value: None },
    });
    Function {
        callable_body,
        gc_effect: GcEffect::Managed,
        signature: ScoopAbiSignature::new(
            Vec::new(),
            AbiReturn::UnitVoid,
            CallingConvention::Cdecl,
        ),
        call_targets: CallTargets::default(),
        safepoints: SafepointIdentities::default(),
        locals: Arena::new(),
        temps: Arena::new(),
        blocks,
        entry,
    }
}

fn generated_body_identity(
    unit: PersistentInitializationUnitId,
    role: InitializationCallableRole,
) -> CallableBodyIdentity {
    let generated =
        PersistentGeneratedCallableId::from_key(&GeneratedCallableKey::Initialization {
            unit,
            role,
        })
        .unwrap();
    CallableBodyIdentity::for_generated_callable(generated).unwrap()
}

fn storage_global(
    identity: StaticStorageIdentity,
    ty: LirType,
    scan: RefScan,
    layout_name: &str,
) -> Global {
    Global {
        address_kind: PointerKind::Raw,
        scan,
        init: GlobalInit::Storage {
            identity,
            layout: LayoutIdentity::managed_value(
                exact_type(layout_name),
                LirTargetProfile::DARWIN_AARCH64,
                MaterializationRoot::cone_owned(),
            )
            .unwrap()
            .into(),
            ty,
            initial_state: LirStaticInitialState::ZeroedForRuntimeUnit,
            thread_local: false,
        },
    }
}

fn replace_storage_identity(global: &mut Global, replacement: StaticStorageIdentity) {
    let GlobalInit::Storage { identity, .. } = &mut global.init else {
        unreachable!()
    };
    *identity = replacement;
}

fn unit_identity(key: InitializationUnitKey) -> crate::InitializationUnitIdentityRecord {
    CborIdentityRecord::from_key(key).unwrap()
}

fn property(name: &str) -> PersistentPropertyId {
    PersistentPropertyId::from_source_declaration(&SourceDeclarationKey::property(
        source_site(),
        CanonicalIdentifier::new(name).unwrap(),
    ))
    .unwrap()
}

fn nominal(name: &str) -> PersistentTypeId {
    PersistentTypeId::from_source_declaration(&SourceDeclarationKey::nominal(
        source_site(),
        CanonicalIdentifier::new(name).unwrap(),
        SourceNominalKind::Object,
        0,
    ))
    .unwrap()
}

fn exact_type(name: &str) -> PersistentExactTypeId {
    PersistentExactTypeId::from_key(&ExactTypeKey::Nominal(nominal(name))).unwrap()
}

fn source_site() -> SourceDeclarationSite {
    SourceDeclarationSite::new(
        ConeIdentity::SINGLE_FILE,
        PackagePath::root(),
        DefinitionOwnerChain::top_level(),
        DeclarationScope::ConeWide,
    )
    .unwrap()
}

mod projections;
