use la_arena::Arena;
use scoop_identity::{
    CallableBodyKey, CanonicalIdentifier, CborIdentityRecord, ConeIdentity, DeclarationScope,
    DefinitionAtomRole, DefinitionAtomSubkey, DefinitionOwnerChain, DigestNodeId, DigestNodeKey,
    DigestPatchIntentKey, DigestSemanticFieldRole, ExactTypeKey, GeneratedCallableKey,
    InitializationCallableRole, InitializationUnitKey, LinkageClass, ObjectDefinitionAtomKey,
    ObjectDefinitionPlanKey, PackagePath, PersistentCallableBodyId, PersistentExactTypeId,
    PersistentGeneratedCallableId, PersistentInitializationUnitId, PersistentPropertyId,
    PersistentStaticStorageId, PersistentSymbolKey, PersistentSymbolRequest,
    PersistentSymbolRequestTable, PersistentTypeId, PropertyOwner, RuntimeIdentityRecord,
    SourceDeclarationKey, SourceDeclarationSite, SourceNominalKind, StrongCallableDefinitionOwner,
    StrongDefinitionEntity, StrongDefinitionRole,
};

use super::*;
use crate::{
    CanonicalLirFoundation, Global, GlobalInit, LayoutIdentity, LirStaticInitialState,
    LirTargetProfile, LirType, MaterializationRoot, PointerKind, RefScan, StaticStorageIdentity,
    StrongStaticStorageSemanticPlanSetV1,
};

#[test]
fn joins_unit_storage_callable_and_digest_relations() {
    let fixture = Fixture::new(Options::default());

    let plans = fixture.build().unwrap();
    let plan = &plans.registrations()[0];

    assert_eq!(plans.producer(), ConeIdentity::SINGLE_FILE);
    assert_eq!(plan.semantic().unit(), fixture.unit);
    assert_eq!(
        plan.registration_symbol().key(),
        PersistentSymbolKey::InitializationRegistration(fixture.unit)
    );
    assert_eq!(
        plan.cell_symbol().key(),
        PersistentSymbolKey::InitializationCell(fixture.unit)
    );
    assert_eq!(
        plan.descriptor_symbol().key(),
        PersistentSymbolKey::InitializationDescriptor(fixture.unit)
    );
    assert_ne!(
        plan.registration_definition_plan(),
        plan.cell_definition_plan()
    );
    assert_ne!(
        plan.cell_definition_plan(),
        plan.descriptor_definition_plan()
    );
    assert_eq!(plan.storage().storage(), fixture.storage);
    assert_eq!(
        plan.storage().storage_symbol().key(),
        PersistentSymbolKey::StaticStorage(fixture.storage)
    );
    assert_eq!(plan.failure_root().storage(), fixture.failure_root);
    assert_eq!(plan.initializer().body(), fixture.initializer);
    assert_eq!(plan.ensure().body(), fixture.ensure);
    assert_eq!(
        plan.schedule().gateway().unwrap().body(),
        fixture.gateway.unwrap()
    );
    assert!(plan.schedule().gateway_definition_patch().is_some());
    assert_eq!(
        fixture
            .foundation
            .definition_atoms()
            .iter()
            .find(|atom| atom.id() == plan.diagnostic_atom())
            .unwrap()
            .key()
            .role(),
        DefinitionAtomRole::AddressTakenConstant
    );
    assert_eq!(
        node(&fixture.digests, plan.registration_fingerprint_node())
            .direct_inputs()
            .len(),
        4
    );
}

#[test]
fn lazy_unit_has_no_gateway_input_or_patch() {
    let fixture = Fixture::new(Options {
        lazy: true,
        ..Options::default()
    });

    let plans = fixture.build().unwrap();
    let plan = &plans.registrations()[0];

    assert_eq!(
        plan.schedule(),
        &StrongInitializationRegistrationSchedulePlanV1::LazyAccess
    );
    assert_eq!(plan.schedule().gateway(), None);
    assert_eq!(
        node(&fixture.digests, plan.registration_fingerprint_node())
            .direct_inputs()
            .len(),
        3
    );
}

#[test]
fn requires_complete_unit_and_referenced_registration_coverage() {
    assert!(matches!(
        Fixture::new(Options {
            omit_unit_registration: true,
            ..Options::default()
        })
        .build(),
        Err(StrongInitializationUnitRegistrationPlanBuildError::UnitSet {
            expected,
            actual
        }) if expected.len() == 1 && actual.is_empty()
    ));
    assert!(matches!(
        Fixture::new(Options {
            omit_storage_registration: true,
            ..Options::default()
        })
        .build(),
        Err(
            StrongInitializationUnitRegistrationPlanBuildError::MissingStaticStorageRegistration(_)
        )
    ));
    assert!(matches!(
        Fixture::new(Options {
            omit_callable_registration: true,
            ..Options::default()
        })
        .build(),
        Err(StrongInitializationUnitRegistrationPlanBuildError::MissingCallableRegistration(_))
    ));
}

#[test]
fn requires_all_three_distinct_definition_surfaces_and_leaf_nodes() {
    assert!(matches!(
        Fixture::new(Options {
            omit_cell_symbol: true,
            ..Options::default()
        })
        .build(),
        Err(StrongInitializationUnitRegistrationPlanBuildError::MissingSymbol(_))
    ));
    assert!(matches!(
        Fixture::new(Options {
            omit_descriptor_primary: true,
            ..Options::default()
        })
        .build(),
        Err(StrongInitializationUnitRegistrationPlanBuildError::PrimaryAtomSet {
            actual,
            ..
        }) if actual.is_empty()
    ));
    assert!(matches!(
        Fixture::new(Options {
            omit_descriptor_object: true,
            ..Options::default()
        })
        .build(),
        Err(StrongInitializationUnitRegistrationPlanBuildError::MissingDigestNode(_))
    ));
    assert!(matches!(
        Fixture::new(Options {
            cell_object_input: true,
            ..Options::default()
        })
        .build(),
        Err(
            StrongInitializationUnitRegistrationPlanBuildError::ObjectLeafInputs {
                leaf: InitializationObjectLeafV1::Cell,
                ..
            }
        )
    ));
    assert!(matches!(
        Fixture::new(Options {
            omit_diagnostic_atom: true,
            ..Options::default()
        })
        .build(),
        Err(StrongInitializationUnitRegistrationPlanBuildError::AssociatedAtomSet {
            actual,
            ..
        }) if actual.is_empty()
    ));
}

#[test]
fn requires_exact_inputs_and_schedule_specific_patch_writers() {
    assert!(matches!(
        Fixture::new(Options {
            omit_descriptor_input: true,
            ..Options::default()
        })
        .build(),
        Err(StrongInitializationUnitRegistrationPlanBuildError::DirectInputs { .. })
    ));
    assert!(matches!(
        Fixture::new(Options {
            omit_registration_patch: true,
            ..Options::default()
        })
        .build(),
        Err(StrongInitializationUnitRegistrationPlanBuildError::PatchSet { .. })
    ));
    assert!(matches!(
        Fixture::new(Options {
            omit_gateway_patch: true,
            ..Options::default()
        })
        .build(),
        Err(StrongInitializationUnitRegistrationPlanBuildError::GatewayPatchSet { .. })
    ));
    assert!(matches!(
        Fixture::new(Options {
            lazy: true,
            unexpected_lazy_gateway_patch: true,
            ..Options::default()
        })
        .build(),
        Err(StrongInitializationUnitRegistrationPlanBuildError::GatewayPatchSet { .. })
    ));
}

#[derive(Clone, Copy, Default)]
struct Options {
    lazy: bool,
    omit_unit_registration: bool,
    omit_storage_registration: bool,
    omit_callable_registration: bool,
    omit_cell_symbol: bool,
    omit_descriptor_primary: bool,
    omit_descriptor_object: bool,
    omit_diagnostic_atom: bool,
    cell_object_input: bool,
    omit_descriptor_input: bool,
    omit_registration_patch: bool,
    omit_gateway_patch: bool,
    unexpected_lazy_gateway_patch: bool,
}

struct DefinitionArtifacts {
    definition: CborIdentityRecord<scoop_identity::ObjectDefinitionPlanId, ObjectDefinitionPlanKey>,
    primary: CborIdentityRecord<scoop_identity::ObjectDefinitionAtomId, ObjectDefinitionAtomKey>,
}

struct CallableArtifacts {
    body: RuntimeIdentityRecord<PersistentCallableBodyId>,
    body_definition: DefinitionArtifacts,
    registration: DefinitionArtifacts,
}

struct Fixture {
    foundation: OdrFreeLirFoundation,
    identities: StrongRegistrationIdentitySurfaceV1,
    semantics: StrongInitializationUnitSemanticPlanSetV1,
    digests: StrongDigestFinalizationPlanV1,
    unit: PersistentInitializationUnitId,
    storage: PersistentStaticStorageId,
    failure_root: PersistentStaticStorageId,
    initializer: PersistentCallableBodyId,
    ensure: PersistentCallableBodyId,
    gateway: Option<PersistentCallableBodyId>,
}

impl Fixture {
    fn new(options: Options) -> Self {
        let unit_record = CborIdentityRecord::from_key(InitializationUnitKey::TopLevelProperty(
            property("value"),
        ))
        .unwrap();
        let unit = unit_record.id();
        let (globals, storage, failure_root) = static_globals(unit);
        let static_storages = StrongStaticStorageSemanticPlanSetV1::from_parts(
            ConeIdentity::SINGLE_FILE,
            LirTargetProfile::DARWIN_AARCH64,
            &globals,
            &crate::StructDefs::default(),
            &crate::EnumDefs::default(),
        )
        .unwrap();

        let initializer = generated_body(unit, InitializationCallableRole::Initializer);
        let ensure = generated_body(unit, InitializationCallableRole::Ensure);
        let gateway = (!options.lazy).then(|| {
            RuntimeIdentityRecord::from_key(&CallableBodyKey::initialization_startup_gateway(unit))
                .unwrap()
        });
        let callables = [initializer.clone(), ensure.clone()]
            .into_iter()
            .chain(gateway.clone())
            .map(callable_artifacts)
            .collect::<Vec<_>>();
        let initializer_id = initializer.id();
        let ensure_id = ensure.id();
        let gateway_id = gateway.as_ref().map(RuntimeIdentityRecord::id);
        let semantics = StrongInitializationUnitSemanticPlanSetV1 {
            static_storages,
            units: vec![StrongInitializationUnitSemanticPlanV1 {
                unit,
                diagnostic_path: "top-level:value".to_string(),
                schedule: match gateway_id {
                    Some(gateway) => StrongInitializationSchedulePlanV1::EagerStartup { gateway },
                    None => StrongInitializationSchedulePlanV1::LazyAccess,
                },
                storage,
                failure_root,
                initializer: initializer_id,
                ensure: ensure_id,
                dependencies: Vec::new(),
            }],
        };

        let cell = definition_artifacts(
            StrongDefinitionEntity::initialization_unit(unit),
            StrongDefinitionRole::InitializationCell,
        );
        let descriptor = definition_artifacts(
            StrongDefinitionEntity::initialization_unit(unit),
            StrongDefinitionRole::InitializationDescriptor,
        );
        let diagnostic_atom = CborIdentityRecord::from_key(ObjectDefinitionAtomKey::new(
            descriptor.definition.id(),
            DefinitionAtomRole::AddressTakenConstant,
            DefinitionAtomSubkey::InitializationUnit(unit),
        ))
        .unwrap();
        let registration = definition_artifacts(
            StrongDefinitionEntity::initialization_unit(unit),
            StrongDefinitionRole::InitializationRegistration,
        );
        let storage_registrations = [storage, failure_root].map(|storage| {
            definition_artifacts(
                StrongDefinitionEntity::static_storage(storage),
                StrongDefinitionRole::RootRegistration,
            )
        });

        let mut canonical = CanonicalLirFoundation::empty();
        canonical
            .set_static_storages(
                globals
                    .iter()
                    .map(|(_, global)| match &global.init {
                        GlobalInit::Storage { identity, .. } => identity.identity_record().clone(),
                        _ => unreachable!(),
                    })
                    .collect(),
            )
            .unwrap();
        canonical
            .set_callable_bodies(callables.iter().map(|item| item.body.clone()).collect())
            .unwrap();
        canonical
            .set_definition_plans(
                [cell.definition.clone(), descriptor.definition.clone()]
                    .into_iter()
                    .chain(
                        (!options.omit_unit_registration).then(|| registration.definition.clone()),
                    )
                    .chain(
                        storage_registrations
                            .iter()
                            .enumerate()
                            .filter(|(index, _)| {
                                !(options.omit_storage_registration && *index == 0)
                            })
                            .map(|(_, item)| item.definition.clone()),
                    )
                    .chain(
                        callables
                            .iter()
                            .flat_map(|item| {
                                [
                                    item.body_definition.definition.clone(),
                                    item.registration.definition.clone(),
                                ]
                            })
                            .filter(|definition| {
                                !(options.omit_callable_registration
                                    && definition.id() == callables[0].registration.definition.id())
                            }),
                    )
                    .collect(),
            )
            .unwrap();
        canonical
            .set_definition_atoms(
                [cell.primary.clone()]
                    .into_iter()
                    .chain((!options.omit_descriptor_primary).then(|| descriptor.primary.clone()))
                    .chain((!options.omit_diagnostic_atom).then_some(diagnostic_atom))
                    .chain((!options.omit_unit_registration).then(|| registration.primary.clone()))
                    .chain(
                        storage_registrations
                            .iter()
                            .enumerate()
                            .filter(|(index, _)| {
                                !(options.omit_storage_registration && *index == 0)
                            })
                            .map(|(_, item)| item.primary.clone()),
                    )
                    .chain(
                        callables
                            .iter()
                            .flat_map(|item| {
                                [
                                    item.body_definition.primary.clone(),
                                    item.registration.primary.clone(),
                                ]
                            })
                            .filter(|atom| {
                                !(options.omit_callable_registration
                                    && atom.key().plan()
                                        == callables[0].registration.definition.id())
                            }),
                    )
                    .collect(),
            )
            .unwrap();
        let symbols = [
            symbol(PersistentSymbolKey::InitializationDescriptor(unit)),
            symbol(PersistentSymbolKey::InitializationRegistration(unit)),
        ]
        .into_iter()
        .chain(
            (!options.omit_cell_symbol)
                .then(|| symbol(PersistentSymbolKey::InitializationCell(unit))),
        )
        .chain([storage, failure_root].into_iter().flat_map(|storage| {
            [
                symbol(PersistentSymbolKey::StaticStorage(storage)),
                symbol(PersistentSymbolKey::RootRegistration(storage)),
            ]
        }))
        .chain(callables.iter().flat_map(|item| {
            [
                symbol(PersistentSymbolKey::CallableBody(item.body.id())),
                symbol(PersistentSymbolKey::CallableRegistration(item.body.id())),
            ]
        }))
        .filter(|request| {
            !options.omit_unit_registration
                || request.key() != PersistentSymbolKey::InitializationRegistration(unit)
        })
        .filter(|request| {
            !options.omit_storage_registration
                || request.key() != PersistentSymbolKey::RootRegistration(storage)
        })
        .filter(|request| {
            !options.omit_callable_registration
                || request.key() != PersistentSymbolKey::CallableRegistration(initializer_id)
        })
        .collect();
        canonical.set_symbol_requests(PersistentSymbolRequestTable::new(symbols).unwrap());

        let foundation =
            OdrFreeLirFoundation::try_new(ConeIdentity::SINGLE_FILE, canonical).unwrap();
        let digests = digest_plan(
            &foundation,
            &cell,
            &descriptor,
            &registration,
            &storage_registrations,
            &callables,
            options,
        );
        let identities =
            StrongRegistrationIdentitySurfaceV1::from_foundation(&foundation, &digests).unwrap();
        Self {
            foundation,
            identities,
            semantics,
            digests,
            unit,
            storage,
            failure_root,
            initializer: initializer_id,
            ensure: ensure_id,
            gateway: gateway_id,
        }
    }

    fn build(
        &self,
    ) -> Result<
        StrongInitializationUnitRegistrationPlanSetV1,
        StrongInitializationUnitRegistrationPlanBuildError,
    > {
        StrongInitializationUnitRegistrationPlanSetV1::new(
            &self.foundation,
            &self.identities,
            &self.semantics,
            &self.digests,
        )
    }
}

fn digest_plan(
    foundation: &OdrFreeLirFoundation,
    cell: &DefinitionArtifacts,
    descriptor: &DefinitionArtifacts,
    registration: &DefinitionArtifacts,
    storages: &[DefinitionArtifacts; 2],
    callables: &[CallableArtifacts],
    options: Options,
) -> StrongDigestFinalizationPlanV1 {
    let auxiliary_input = options.cell_object_input.then(|| {
        DigestNodeV1::new(
            DigestNodeKey::source_signature(callables[0].body.id()),
            Vec::new(),
            Vec::new(),
        )
        .unwrap()
    });
    let cell_object = DigestNodeV1::new(
        DigestNodeKey::object_definition(cell.primary.id()),
        auxiliary_input
            .as_ref()
            .map(DigestInputRefV1::from_node)
            .into_iter()
            .collect(),
        Vec::new(),
    )
    .unwrap();
    let descriptor_object = (!options.omit_descriptor_primary && !options.omit_descriptor_object)
        .then(|| {
            DigestNodeV1::new(
                DigestNodeKey::object_definition(descriptor.primary.id()),
                Vec::new(),
                Vec::new(),
            )
            .unwrap()
        });

    let unit_registration_present = !options.omit_unit_registration;
    let registration_object = unit_registration_present.then(|| {
        DigestNodeV1::new(
            DigestNodeKey::object_definition(registration.primary.id()),
            Vec::new(),
            Vec::new(),
        )
        .unwrap()
    });
    let callable_body_nodes = callables
        .iter()
        .enumerate()
        .map(|(index, item)| {
            let key = DigestNodeKey::object_definition(item.body_definition.primary.id());
            let source = DigestNodeId::from_key(&key).unwrap();
            let is_gateway = !options.lazy && index == 2;
            let patches = if unit_registration_present
                && ((is_gateway && !options.omit_gateway_patch)
                    || (!is_gateway
                        && options.unexpected_lazy_gateway_patch
                        && item.body.id() == callables[1].body.id()))
            {
                vec![DigestPatchIntentKey::new(
                    source,
                    registration.definition.id(),
                    DefinitionAtomRole::Primary,
                    DigestSemanticFieldRole::GatewayDefinition,
                )]
            } else {
                Vec::new()
            };
            DigestNodeV1::new(key, Vec::new(), patches).unwrap()
        })
        .collect::<Vec<_>>();

    let mut nodes = vec![cell_object.clone()];
    nodes.extend(auxiliary_input);
    nodes.extend(descriptor_object.clone());
    nodes.extend(registration_object.clone());
    nodes.extend(callable_body_nodes.iter().cloned());
    let mut image_inputs = Vec::new();
    for (index, storage) in storages.iter().enumerate() {
        if options.omit_storage_registration && index == 0 {
            continue;
        }
        let strong = DigestNodeV1::new(
            DigestNodeKey::strong_registration(storage.definition.id()),
            Vec::new(),
            Vec::new(),
        )
        .unwrap();
        image_inputs.push(DigestInputRefV1::from_node(&strong));
        nodes.push(strong);
    }
    for (index, callable) in callables.iter().enumerate() {
        if options.omit_callable_registration && index == 0 {
            continue;
        }
        let strong = DigestNodeV1::new(
            DigestNodeKey::strong_registration(callable.registration.definition.id()),
            Vec::new(),
            Vec::new(),
        )
        .unwrap();
        image_inputs.push(DigestInputRefV1::from_node(&strong));
        nodes.push(strong);
    }
    if unit_registration_present {
        let strong_key = DigestNodeKey::strong_registration(registration.definition.id());
        let strong_source = DigestNodeId::from_key(&strong_key).unwrap();
        let mut inputs = vec![
            DigestInputRefV1::from_node(registration_object.as_ref().unwrap()),
            DigestInputRefV1::from_node(&cell_object),
        ];
        if !options.omit_descriptor_input {
            if let Some(descriptor) = &descriptor_object {
                inputs.push(DigestInputRefV1::from_node(descriptor));
            }
        }
        if !options.lazy {
            inputs.push(DigestInputRefV1::from_node(&callable_body_nodes[2]));
        }
        let strong = DigestNodeV1::new(
            strong_key,
            inputs,
            if options.omit_registration_patch {
                Vec::new()
            } else {
                vec![DigestPatchIntentKey::new(
                    strong_source,
                    registration.definition.id(),
                    DefinitionAtomRole::Primary,
                    DigestSemanticFieldRole::RegistrationDefinition,
                )]
            },
        )
        .unwrap();
        image_inputs.push(DigestInputRefV1::from_node(&strong));
        nodes.push(strong);
    }
    nodes.push(
        DigestNodeV1::new(
            DigestNodeKey::runtime_image(ConeIdentity::SINGLE_FILE),
            image_inputs,
            Vec::new(),
        )
        .unwrap(),
    );
    StrongDigestFinalizationPlanV1::new(nodes, foundation).unwrap()
}

fn definition_artifacts(
    entity: StrongDefinitionEntity,
    role: StrongDefinitionRole,
) -> DefinitionArtifacts {
    let definition = CborIdentityRecord::from_key(
        ObjectDefinitionPlanKey::strong(ConeIdentity::SINGLE_FILE, entity, role).unwrap(),
    )
    .unwrap();
    let primary = CborIdentityRecord::from_key(ObjectDefinitionAtomKey::new(
        definition.id(),
        DefinitionAtomRole::Primary,
        DefinitionAtomSubkey::Singleton,
    ))
    .unwrap();
    DefinitionArtifacts {
        definition,
        primary,
    }
}

fn callable_artifacts(body: RuntimeIdentityRecord<PersistentCallableBodyId>) -> CallableArtifacts {
    CallableArtifacts {
        body_definition: definition_artifacts(
            StrongDefinitionEntity::callable_body(body.id()),
            StrongDefinitionRole::CallableBody,
        ),
        registration: definition_artifacts(
            StrongDefinitionEntity::callable_body(body.id()),
            StrongDefinitionRole::CallableRegistration,
        ),
        body,
    }
}

fn generated_body(
    unit: PersistentInitializationUnitId,
    role: InitializationCallableRole,
) -> RuntimeIdentityRecord<PersistentCallableBodyId> {
    let generated =
        PersistentGeneratedCallableId::from_key(&GeneratedCallableKey::Initialization {
            unit,
            role,
        })
        .unwrap();
    RuntimeIdentityRecord::from_key(&CallableBodyKey::strong(
        StrongCallableDefinitionOwner::GeneratedCallable(generated),
    ))
    .unwrap()
}

fn static_globals(
    unit: PersistentInitializationUnitId,
) -> (
    Arena<Global>,
    PersistentStaticStorageId,
    PersistentStaticStorageId,
) {
    let storage_identity = StaticStorageIdentity::property_backing(
        PropertyOwner::Property(property("value")),
        MaterializationRoot::cone_owned(),
    )
    .unwrap();
    let failure_identity =
        StaticStorageIdentity::initialization_failure_root(unit, MaterializationRoot::cone_owned())
            .unwrap();
    let storage = storage_identity.identity_record().id();
    let failure_root = failure_identity.identity_record().id();
    let mut globals = Arena::new();
    globals.alloc(storage_global(
        storage_identity,
        exact_type("Value"),
        LirType::I64,
        RefScan::None,
    ));
    globals.alloc(storage_global(
        failure_identity,
        exact_type("Failure"),
        crate::MANAGED_PTR,
        RefScan::References(vec![0]),
    ));
    (globals, storage, failure_root)
}

fn storage_global(
    identity: StaticStorageIdentity,
    exact_type: PersistentExactTypeId,
    ty: LirType,
    scan: RefScan,
) -> Global {
    Global {
        address_kind: PointerKind::Raw,
        scan,
        init: GlobalInit::Storage {
            identity,
            layout: LayoutIdentity::managed_value(
                exact_type,
                LirTargetProfile::DARWIN_AARCH64,
                MaterializationRoot::cone_owned(),
            )
            .unwrap(),
            ty,
            initial_state: LirStaticInitialState::ZeroedForRuntimeUnit,
            thread_local: false,
        },
    }
}

fn property(name: &str) -> PersistentPropertyId {
    PersistentPropertyId::from_source_declaration(&SourceDeclarationKey::property(
        source_site(),
        CanonicalIdentifier::new(name).unwrap(),
    ))
    .unwrap()
}

fn exact_type(name: &str) -> PersistentExactTypeId {
    let nominal = PersistentTypeId::from_source_declaration(&SourceDeclarationKey::nominal(
        source_site(),
        CanonicalIdentifier::new(name).unwrap(),
        SourceNominalKind::Struct,
        0,
    ))
    .unwrap();
    PersistentExactTypeId::from_key(&ExactTypeKey::Nominal(nominal)).unwrap()
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

fn symbol(key: PersistentSymbolKey) -> PersistentSymbolRequest {
    PersistentSymbolRequest::new(key, LinkageClass::ConeStrong).unwrap()
}

fn node(plan: &StrongDigestFinalizationPlanV1, id: DigestNodeId) -> &DigestNodeV1 {
    plan.nodes().iter().find(|node| node.id() == id).unwrap()
}

mod dependencies_v2;
