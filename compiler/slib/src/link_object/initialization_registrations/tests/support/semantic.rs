mod module;

use module::semantic_module;

use la_arena::Arena;
use scoop_identity::{
    CanonicalIdentifier, CborIdentityRecord, DeclarationScope, DefinitionAtomRole,
    DefinitionAtomSubkey, DefinitionOwnerChain, DigestNodeId, DigestNodeKey, DigestPatchIntentKey,
    DigestSemanticFieldRole, ExactTypeKey, GeneratedCallableKey, InitializationCallableRole,
    InitializationUnitKey, ObjectDefinitionAtomKey, ObjectDefinitionPlanId,
    ObjectDefinitionPlanKey, PackagePath, PersistentExactTypeId, PersistentGeneratedCallableId,
    PersistentPropertyId, PersistentSymbolKey, PersistentSymbolRequest,
    PersistentSymbolRequestTable, PersistentTypeId, PropertyOwner, SourceDeclarationKey,
    SourceDeclarationSite, SourceNominalKind, StrongDefinitionEntity, StrongDefinitionRole,
};
use scoop_lir::{
    AbiReturn, BasicBlock, CallTargets, CallableBodyIdentity, CallingConvention,
    CanonicalCAbiMetadata, CanonicalLirFoundation, ConeLirFoundation, DigestFinalizationPlanV1,
    DigestInputRefV1, DigestNodeV1, EnumDefs, ExternFunctions, ExternalTypeDescriptor, Function,
    GcEffect, Global, GlobalInit, InitializationSchedule, InitializationUnit,
    InitializationUnitKind, Layout, LayoutIdentity, LayoutKind, LirMeta, LirStaticInitialState,
    LirTargetProfile, LirType, LocalFunctionIdentities, LocalFunctionRef, MaterializationRoot,
    Module, NativeExternalMetadata, NativeGlobalBridges, PointerKind, RefScan,
    RegistrationIdentitySurfaceV1, SafepointIdentities, ScoopAbiSignature, StaticStorageIdentity,
    StrongCallableRegistrationPlanSetV1, StrongInitializationUnitRegistrationPlanSetV1,
    StrongInitializationUnitSemanticPlanSetV1, StrongSafepointSemanticPlanSetV1, StructDefs,
    Terminator, TypeDescriptorRef, WellKnownTypeDescriptors,
};

pub(super) struct SemanticInputs {
    pub(super) foundation: ConeLirFoundation,
    pub(super) definitions: Vec<ObjectDefinitionPlanId>,
    pub(super) digest_plan: DigestFinalizationPlanV1,
    pub(super) plan: StrongInitializationUnitRegistrationPlanSetV1,
    pub(super) callable_plan: StrongCallableRegistrationPlanSetV1,
    pub(super) safepoints: StrongSafepointSemanticPlanSetV1,
}

pub(super) fn inputs(lazy: bool) -> SemanticInputs {
    let module = semantic_module(lazy);
    let semantics = StrongInitializationUnitSemanticPlanSetV1::from_module(&module).unwrap();
    let semantic = &semantics.units()[0];
    let unit = semantic.unit();

    let cell = definition_artifacts(
        StrongDefinitionEntity::initialization_unit(unit),
        StrongDefinitionRole::InitializationCell,
    );
    let registration = definition_artifacts(
        StrongDefinitionEntity::initialization_unit(unit),
        StrongDefinitionRole::InitializationRegistration,
    );
    let diagnostic_atom = CborIdentityRecord::from_key(ObjectDefinitionAtomKey::new(
        registration.definition.id(),
        DefinitionAtomRole::AddressTakenConstant,
        DefinitionAtomSubkey::InitializationUnit(unit),
    ))
    .unwrap();
    let storages = [semantic.storage(), semantic.failure_root()].map(|storage| StorageArtifacts {
        storage,
        storage_definition: definition_artifacts(
            StrongDefinitionEntity::static_storage(storage),
            StrongDefinitionRole::StaticStorage,
        ),
        registration: definition_artifacts(
            StrongDefinitionEntity::static_storage(storage),
            StrongDefinitionRole::RootRegistration,
        ),
    });
    let callables = module
        .functions
        .iter()
        .map(|function| CallableArtifacts {
            body: function.callable_body.id(),
            body_record: function.callable_body.identity_record().clone(),
            body_definition: definition_artifacts(
                StrongDefinitionEntity::callable_body(function.callable_body.id()),
                StrongDefinitionRole::CallableBody,
            ),
            registration: definition_artifacts(
                StrongDefinitionEntity::callable_body(function.callable_body.id()),
                StrongDefinitionRole::CallableRegistration,
            ),
        })
        .collect::<Vec<_>>();

    let mut canonical = CanonicalLirFoundation::empty();
    canonical
        .set_static_storages(
            module
                .globals
                .iter()
                .map(|(_, global)| match &global.init {
                    GlobalInit::Storage { identity, .. } => identity.identity_record().clone(),
                    GlobalInit::CString { .. }
                    | GlobalInit::StringConst { .. }
                    | GlobalInit::RawStorage { .. }
                    | GlobalInit::ImportedStorage { .. } => unreachable!(),
                })
                .collect(),
        )
        .unwrap();
    canonical
        .set_callable_bodies(
            callables
                .iter()
                .map(|callable| callable.body_record.clone())
                .collect(),
        )
        .unwrap();
    canonical
        .set_definition_plans(
            [cell.definition.clone(), registration.definition.clone()]
                .into_iter()
                .chain(storages.iter().flat_map(|storage| {
                    [
                        storage.storage_definition.definition.clone(),
                        storage.registration.definition.clone(),
                    ]
                }))
                .chain(callables.iter().flat_map(|callable| {
                    [
                        callable.body_definition.definition.clone(),
                        callable.registration.definition.clone(),
                    ]
                }))
                .collect(),
        )
        .unwrap();
    canonical
        .set_definition_atoms(
            [
                cell.primary.clone(),
                diagnostic_atom,
                registration.primary.clone(),
            ]
            .into_iter()
            .chain(storages.iter().flat_map(|storage| {
                [
                    storage.storage_definition.primary.clone(),
                    storage.registration.primary.clone(),
                ]
            }))
            .chain(callables.iter().flat_map(|callable| {
                [
                    callable.body_definition.primary.clone(),
                    callable.registration.primary.clone(),
                ]
            }))
            .collect(),
        )
        .unwrap();
    canonical.set_symbol_requests(
        PersistentSymbolRequestTable::new(
            [
                symbol(PersistentSymbolKey::InitializationCell(unit)),
                symbol(PersistentSymbolKey::InitializationRegistration(unit)),
            ]
            .into_iter()
            .chain(storages.iter().flat_map(|storage| {
                [
                    symbol(PersistentSymbolKey::StaticStorage(storage.storage)),
                    symbol(PersistentSymbolKey::RootRegistration(storage.storage)),
                ]
            }))
            .chain(callables.iter().flat_map(|callable| {
                [
                    symbol(PersistentSymbolKey::CallableBody(callable.body)),
                    symbol(PersistentSymbolKey::CallableRegistration(callable.body)),
                ]
            }))
            .collect(),
        )
        .unwrap(),
    );

    let foundation =
        ConeLirFoundation::try_new(scoop_lir::ConeIdentity::SINGLE_FILE, canonical).unwrap();
    let digest_plan = digest_plan(
        &foundation,
        semantic.schedule().gateway(),
        &registration,
        &callables,
    );
    let identities = RegistrationIdentitySurfaceV1::from_foundation(&foundation).unwrap();
    let callable_plan = StrongCallableRegistrationPlanSetV1::new(
        &foundation,
        &identities,
        scoop_lir::StrongCallableRuntimeScanPlanSetV1::from_foundation_without_scans(&foundation)
            .unwrap(),
        &digest_plan,
    )
    .unwrap();
    let plan = StrongInitializationUnitRegistrationPlanSetV1::new(
        &foundation,
        &identities,
        &semantics,
        &digest_plan,
    )
    .unwrap();
    let safepoints = StrongSafepointSemanticPlanSetV1::from_module(&module).unwrap();
    let definitions = [cell.definition.id(), registration.definition.id()]
        .into_iter()
        .chain(storages.iter().flat_map(|storage| {
            [
                storage.storage_definition.definition.id(),
                storage.registration.definition.id(),
            ]
        }))
        .chain(callables.iter().flat_map(|callable| {
            [
                callable.body_definition.definition.id(),
                callable.registration.definition.id(),
            ]
        }))
        .collect();
    SemanticInputs {
        foundation,
        definitions,
        digest_plan,
        plan,
        callable_plan,
        safepoints,
    }
}

struct DefinitionArtifacts {
    definition: CborIdentityRecord<scoop_lir::ObjectDefinitionPlanId, ObjectDefinitionPlanKey>,
    primary: CborIdentityRecord<scoop_lir::ObjectDefinitionAtomId, ObjectDefinitionAtomKey>,
}

struct StorageArtifacts {
    storage: scoop_lir::PersistentStaticStorageId,
    storage_definition: DefinitionArtifacts,
    registration: DefinitionArtifacts,
}

struct CallableArtifacts {
    body: scoop_lir::PersistentCallableBodyId,
    body_record: scoop_identity::RuntimeIdentityRecord<scoop_lir::PersistentCallableBodyId>,
    body_definition: DefinitionArtifacts,
    registration: DefinitionArtifacts,
}

fn definition_artifacts(
    entity: StrongDefinitionEntity,
    role: StrongDefinitionRole,
) -> DefinitionArtifacts {
    let definition = CborIdentityRecord::from_key(
        ObjectDefinitionPlanKey::strong(scoop_lir::ConeIdentity::SINGLE_FILE, entity, role)
            .unwrap(),
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

fn digest_plan(
    foundation: &ConeLirFoundation,
    gateway: Option<scoop_lir::PersistentCallableBodyId>,
    registration: &DefinitionArtifacts,
    callables: &[CallableArtifacts],
) -> DigestFinalizationPlanV1 {
    let callable_objects = callables
        .iter()
        .map(|callable| {
            let key = DigestNodeKey::object_definition(callable.body_definition.primary.id());
            let id = DigestNodeId::from_key(&key).unwrap();
            let mut patches = vec![DigestPatchIntentKey::new(
                id,
                callable.registration.definition.id(),
                DefinitionAtomRole::Primary,
                DigestSemanticFieldRole::CallableBodyDefinition,
            )];
            if Some(callable.body) == gateway {
                patches.push(DigestPatchIntentKey::new(
                    id,
                    registration.definition.id(),
                    DefinitionAtomRole::Primary,
                    DigestSemanticFieldRole::GatewayDefinition,
                ));
            }
            DigestNodeV1::new(key, Vec::new(), patches).unwrap()
        })
        .collect::<Vec<_>>();
    let image_inputs = callable_objects
        .iter()
        .map(DigestInputRefV1::from_node)
        .collect();
    let mut nodes = callable_objects;
    nodes.push(
        DigestNodeV1::new(
            DigestNodeKey::runtime_image(scoop_lir::ConeIdentity::SINGLE_FILE),
            image_inputs,
            Vec::new(),
        )
        .unwrap(),
    );
    DigestFinalizationPlanV1::new(nodes, foundation).unwrap()
}

fn symbol(key: PersistentSymbolKey) -> PersistentSymbolRequest {
    PersistentSymbolRequest::new(key, scoop_lir::LinkageClass::ConeStrong).unwrap()
}
