//! Checked physical fixtures for initialization registration versions.

use super::fixture_digest::digest_plan;
use super::*;

mod complete;

#[derive(Clone, Copy, Default)]
pub(super) struct Options {
    pub(super) lazy: bool,
    pub(super) omit_unit_registration: bool,
    pub(super) omit_storage_registration: bool,
    pub(super) omit_callable_registration: bool,
    pub(super) omit_cell_symbol: bool,
    pub(super) omit_descriptor_primary: bool,
    pub(super) omit_descriptor_object: bool,
    pub(super) omit_diagnostic_atom: bool,
    pub(super) cell_object_input: bool,
    pub(super) omit_descriptor_input: bool,
    pub(super) omit_registration_patch: bool,
    pub(super) omit_gateway_patch: bool,
    pub(super) unexpected_lazy_gateway_patch: bool,
}

pub(super) struct DefinitionArtifacts {
    pub(super) definition:
        CborIdentityRecord<scoop_identity::ObjectDefinitionPlanId, ObjectDefinitionPlanKey>,
    pub(super) primary:
        CborIdentityRecord<scoop_identity::ObjectDefinitionAtomId, ObjectDefinitionAtomKey>,
}

pub(super) struct CallableArtifacts {
    pub(super) body: RuntimeIdentityRecord<PersistentCallableBodyId>,
    pub(super) body_definition: DefinitionArtifacts,
    pub(super) registration: DefinitionArtifacts,
}

pub(super) struct Fixture {
    pub(super) foundation: OdrFreeLirFoundation,
    pub(super) identities: StrongRegistrationIdentitySurfaceV1,
    pub(super) semantics: StrongInitializationUnitSemanticPlanSetV1,
    pub(super) digests: StrongDigestFinalizationPlanV1,
    pub(super) unit: PersistentInitializationUnitId,
    pub(super) storage: PersistentStaticStorageId,
    pub(super) failure_root: PersistentStaticStorageId,
    pub(super) initializer: PersistentCallableBodyId,
    pub(super) ensure: PersistentCallableBodyId,
    pub(super) gateway: Option<PersistentCallableBodyId>,
}

impl Fixture {
    pub(super) fn new(options: Options) -> Self {
        Self::with_source(options, ConeIdentity::SINGLE_FILE, "value")
    }

    pub(super) fn with_source(options: Options, producer: ConeIdentity, name: &str) -> Self {
        let unit_record = CborIdentityRecord::from_key(InitializationUnitKey::TopLevelProperty(
            source_property(producer, name),
        ))
        .unwrap();
        let unit = unit_record.id();
        let (globals, storage, failure_root) =
            static_globals(unit, source_property(producer, name));
        let static_storages = StrongStaticStorageSemanticPlanSetV1::from_parts(
            producer,
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
            .map(|body| callable_artifacts(producer, body))
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
            producer,
            StrongDefinitionEntity::initialization_unit(unit),
            StrongDefinitionRole::InitializationCell,
        );
        let descriptor = definition_artifacts(
            producer,
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
            producer,
            StrongDefinitionEntity::initialization_unit(unit),
            StrongDefinitionRole::InitializationRegistration,
        );
        let storage_registrations = [storage, failure_root].map(|storage| {
            definition_artifacts(
                producer,
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

        let foundation = OdrFreeLirFoundation::try_new(producer, canonical).unwrap();
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

    pub(super) fn build(
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

fn definition_artifacts(
    producer: ConeIdentity,
    entity: StrongDefinitionEntity,
    role: StrongDefinitionRole,
) -> DefinitionArtifacts {
    let definition = CborIdentityRecord::from_key(
        ObjectDefinitionPlanKey::strong(producer, entity, role).unwrap(),
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

fn callable_artifacts(
    producer: ConeIdentity,
    body: RuntimeIdentityRecord<PersistentCallableBodyId>,
) -> CallableArtifacts {
    CallableArtifacts {
        body_definition: definition_artifacts(
            producer,
            StrongDefinitionEntity::callable_body(body.id()),
            StrongDefinitionRole::CallableBody,
        ),
        registration: definition_artifacts(
            producer,
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
    property: PersistentPropertyId,
) -> (
    Arena<Global>,
    PersistentStaticStorageId,
    PersistentStaticStorageId,
) {
    let storage_identity = StaticStorageIdentity::property_backing(
        PropertyOwner::Property(property),
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
            .unwrap()
            .into(),
            ty,
            initial_state: LirStaticInitialState::ZeroedForRuntimeUnit,
            thread_local: false,
        },
    }
}

pub(super) fn property(name: &str) -> PersistentPropertyId {
    source_property(ConeIdentity::SINGLE_FILE, name)
}

fn source_property(producer: ConeIdentity, name: &str) -> PersistentPropertyId {
    PersistentPropertyId::from_source_declaration(&SourceDeclarationKey::property(
        SourceDeclarationSite::new(
            producer,
            PackagePath::root(),
            DefinitionOwnerChain::top_level(),
            DeclarationScope::ConeWide,
        )
        .unwrap(),
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

pub(super) fn node(plan: &StrongDigestFinalizationPlanV1, id: DigestNodeId) -> &DigestNodeV1 {
    plan.nodes().iter().find(|node| node.id() == id).unwrap()
}
