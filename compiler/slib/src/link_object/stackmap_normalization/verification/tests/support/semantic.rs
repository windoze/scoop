use la_arena::Arena;
use scoop_identity::{
    CanonicalIdentifier, CborIdentityRecord, ConeIdentity, DeclarationScope, DefinitionAtomRole,
    DefinitionAtomSubkey, DefinitionOwnerChain, DigestNodeId, DigestNodeKey, DigestPatchIntentKey,
    DigestSemanticFieldRole, ExactTypeKey, ImmortalObjectKey, ImmortalObjectOwner, LayoutKey,
    LinkageClass, ObjectDefinitionAtomId, ObjectDefinitionAtomKey, ObjectDefinitionPlanId,
    ObjectDefinitionPlanKey, PackagePath, PersistentExactTypeId, PersistentFunctionId,
    PersistentImmortalObjectId, PersistentLayoutId, PersistentPropertyId,
    PersistentSafepointSiteId, PersistentScanId, PersistentStaticStorageId, PersistentSymbolKey,
    PersistentSymbolRequest, PersistentSymbolRequestTable, PersistentTypeId, PropertyOwner,
    RepresentationRole, ScanKey, SourceDeclarationKey, SourceDeclarationSite, SourceNominalKind,
    StrongDefinitionEntity, StrongDefinitionRole, StructuralDefinitionPath,
    StructuralDefinitionSiteRole, StructuralPathSegment,
};
use scoop_lir::{
    AbiReturn, BasicBlock, CallTarget, CallTargets, CallableBodyIdentity, CanonicalCAbiMetadata,
    CanonicalLirFoundation, DecodedStrongRegistrationProductionSurfaceV1, DigestInputRefV1,
    DigestNodeV1, EnumDefs, ExternFunctions, Function, GcEffect, Global, GlobalInit,
    ImmortalObjectIdentity, Instruction, IntrinsicTypeRepresentation, Layout, LayoutIdentity,
    LayoutKind, LirConstantImage, LirMeta, LirStaticInitialState, LirTargetProfile, LirType,
    LocalFunctionIdentities, LocalFunctionRef, ManagedCallDestination, ManagedPollSite,
    ManagedRuntimeFunction, MaterializationRoot, Module, NativeExternalMetadata,
    NativeGlobalBridges, OdrFreeLirFoundation, PointerKind, RefScan, RuntimeTypeMappingRecord,
    SafepointIdentities, SafepointIdentity, SafepointMappingRecord, SafepointSiteRef,
    SafepointSiteRole, ScoopAbiSignature, StatepointLiveSet, StaticStorageIdentity,
    StrongCallableRegistrationPlanSetV1, StrongDigestFinalizationPlanV1,
    StrongImmortalObjectRegistrationPlanSetV1, StrongInitializationUnitRegistrationPlanSetV1,
    StrongRegistrationProductionSurfaceV1, StrongSafepointRegistrationPlanSetV1,
    StrongStaticStorageRegistrationPlanSetV1, StrongTypeRegistrationPlanSetV1, StructDefs,
    Terminator, TypeDescriptor, TypeDescriptorIdentity, TypeDescriptorRef, TypeInstanceShapeV1,
    VoidCallSignature, VtableRecord, WellKnownTypeDescriptors,
};

use super::Corruption;

pub(crate) struct SemanticInputs {
    pub(crate) module: Module,
    pub(crate) foundation: OdrFreeLirFoundation,
    pub(crate) definitions: Vec<ObjectDefinitionPlanId>,
    pub(crate) digest_plan: StrongDigestFinalizationPlanV1,
    pub(crate) registration_plan: StrongSafepointRegistrationPlanSetV1,
    pub(crate) callable_registration_plan: StrongCallableRegistrationPlanSetV1,
    pub(crate) type_registration_plan: StrongTypeRegistrationPlanSetV1,
    pub(crate) immortal_registration_plan: StrongImmortalObjectRegistrationPlanSetV1,
    pub(crate) static_storage_registration_plan: StrongStaticStorageRegistrationPlanSetV1,
    pub(crate) initialization_registration_plan: StrongInitializationUnitRegistrationPlanSetV1,
    pub(crate) registration_production: StrongRegistrationProductionSurfaceV1,
    pub(crate) safepoint_ids: Vec<u64>,
}

pub(crate) fn inputs(corruption: Corruption) -> SemanticInputs {
    let (module, body, safepoints) = semantic_module(corruption);
    let (
        foundation,
        registrations,
        callable_registration,
        type_registration,
        immortal_registration,
        static_storage,
    ) = foundation(&module, &body, &safepoints, corruption);
    let digest_plan = digest_plan(
        &foundation,
        body.id(),
        &registrations,
        &callable_registration,
        &type_registration,
        &immortal_registration,
        &static_storage,
    );
    let registration_production = StrongRegistrationProductionSurfaceV1::from_semantics(
        module.meta.target_profile,
        &foundation,
        &digest_plan,
        scoop_lir::StrongRegistrationIdentitySurfaceV1::from_foundation(&foundation, &digest_plan)
            .unwrap(),
        scoop_lir::StrongCallableRuntimeScanPlanSetV1::from_module(&module).unwrap(),
        scoop_lir::StrongTypeDescriptorSemanticPlanSetV1::from_module(&module).unwrap(),
        scoop_lir::StrongSafepointSemanticPlanSetV1::from_module(&module).unwrap(),
        scoop_lir::StrongImmortalObjectSemanticPlanSetV1::from_module(&module).unwrap(),
        scoop_lir::StrongInitializationUnitSemanticPlanSetV1::from_module(&module).unwrap(),
    )
    .unwrap();
    let registration_plan = registration_production.safepoints().clone();
    let callable_registration_plan = registration_production.callables().clone();
    let type_registration_plan = registration_production.types().clone();
    let immortal_registration_plan = registration_production.immortal_objects().clone();
    let static_storage_registration_plan = registration_production.static_storages().clone();
    let initialization_registration_plan = registration_production.initialization_units().clone();
    let definitions = std::iter::once(definition_plan(body.id()))
        .chain(std::iter::once(callable_registration.plan.id()))
        .chain([
            type_registration.descriptor_plan.id(),
            type_registration.layout_plan.id(),
            type_registration.registration_plan.id(),
        ])
        .chain([
            immortal_registration.object_plan.id(),
            immortal_registration.registration_plan.id(),
        ])
        .chain([
            static_storage.storage_plan.id(),
            static_storage.registration_plan.id(),
            static_storage.layout_plan.id(),
            static_storage.scan_plan.id(),
        ])
        .chain(
            registrations
                .iter()
                .map(|registration| registration.plan.id()),
        )
        .collect();
    let safepoint_ids = safepoints
        .iter()
        .map(|identity| identity.runtime_id().get())
        .collect();
    SemanticInputs {
        module,
        foundation,
        definitions,
        digest_plan,
        registration_plan,
        callable_registration_plan,
        type_registration_plan,
        immortal_registration_plan,
        static_storage_registration_plan,
        initialization_registration_plan,
        registration_production,
        safepoint_ids,
    }
}

#[test]
fn complete_registration_production_uses_the_closed_eight_field_shape() {
    let inputs = inputs(Corruption::None);
    let encoded = scoop_wire::encode(&inputs.registration_production).unwrap();
    assert_eq!(encoded[0], 0xa8);
    let decoded =
        scoop_wire::decode_canonical::<DecodedStrongRegistrationProductionSurfaceV1>(&encoded)
            .unwrap();
    assert_eq!(scoop_wire::encode(&decoded).unwrap(), encoded);

    let validated = decoded
        .validate(
            inputs.module.meta.target_profile,
            &inputs.foundation,
            &inputs.digest_plan,
        )
        .unwrap();
    assert_eq!(validated, inputs.registration_production);

    let old_identity_only =
        scoop_wire::encode(inputs.registration_production.identities()).unwrap();
    assert!(
        scoop_wire::decode_canonical::<DecodedStrongRegistrationProductionSurfaceV1>(
            &old_identity_only,
        )
        .is_err()
    );
}

#[test]
fn registration_production_rejects_a_stale_derived_plan_identity() {
    let inputs = inputs(Corruption::None);

    let stale =
        inputs.registration_production.safepoints().registrations()[0].normalized_stackmap_patch();
    let mut encoded = scoop_wire::encode(&inputs.registration_production).unwrap();
    let offset = encoded
        .windows(stale.as_array().len())
        .position(|bytes| bytes == stale.as_array())
        .unwrap();
    encoded[offset] ^= 1;
    let decoded =
        scoop_wire::decode_canonical::<DecodedStrongRegistrationProductionSurfaceV1>(&encoded)
            .unwrap();

    assert!(
        decoded
            .validate(
                inputs.module.meta.target_profile,
                &inputs.foundation,
                &inputs.digest_plan,
            )
            .is_err()
    );
}

fn semantic_module(
    corruption: Corruption,
) -> (Module, CallableBodyIdentity, Vec<SafepointIdentity>) {
    let body = CallableBodyIdentity::for_function(function_id("stackmapOwner")).unwrap();
    let safepoints = vec![
        SafepointIdentity::new(body.id(), SafepointSiteRole::ManagedPoll, 0).unwrap(),
        SafepointIdentity::new(body.id(), SafepointSiteRole::ManagedPoll, 1).unwrap(),
    ];
    let mut call_targets = CallTargets::default();
    let signature = call_targets.void_signatures.alloc(VoidCallSignature::new(
        Vec::new(),
        scoop_lir::CallingConvention::Cdecl,
    ));
    let target = call_targets.managed_targets.void.alloc(CallTarget {
        destination: ManagedCallDestination::runtime(ManagedRuntimeFunction::Safepoint),
        signature,
    });
    let mut blocks = Arena::new();
    let entry = blocks.alloc(BasicBlock {
        name: "entry".to_string(),
        instructions: vec![
            Instruction::ManagedPoll {
                site: ManagedPollSite {
                    target,
                    safepoint: SafepointSiteRef::from_u32(0),
                    live: StatepointLiveSet::default(),
                },
            },
            Instruction::ManagedPoll {
                site: ManagedPollSite {
                    target,
                    safepoint: SafepointSiteRef::from_u32(1),
                    live: StatepointLiveSet::default(),
                },
            },
        ],
        terminator: Terminator::Return { value: None },
    });
    let function = Function {
        callable_body: body.clone(),
        gc_effect: GcEffect::Managed,
        signature: ScoopAbiSignature::new(
            Vec::new(),
            AbiReturn::UnitVoid,
            scoop_lir::CallingConvention::Cdecl,
        ),
        call_targets,
        safepoints: SafepointIdentities::checked(vec![
            (SafepointSiteRef::from_u32(0), safepoints[0].clone()),
            (SafepointSiteRef::from_u32(1), safepoints[1].clone()),
        ])
        .unwrap(),
        locals: Arena::new(),
        temps: Arena::new(),
        blocks,
        entry,
    };
    let mut local_functions = LocalFunctionIdentities::default();
    let entry = LocalFunctionRef::Managed(local_functions.alloc_managed());
    let mut globals = Arena::new();
    let immortal = globals.alloc(Global {
        address_kind: PointerKind::Managed,
        scan: RefScan::None,
        init: GlobalInit::StringConst {
            identity: ImmortalObjectIdentity::from_key(
                immortal_key(),
                MaterializationRoot::cone_owned(),
            )
            .unwrap(),
            value: "stage3".to_string(),
        },
    });
    let initial_state = if matches!(
        corruption,
        Corruption::StaticZeroedInitialState
            | Corruption::StaticZeroedWritableSection
            | Corruption::StaticSentinelCollision
    ) {
        LirStaticInitialState::ZeroedForRuntimeUnit
    } else if matches!(
        corruption,
        Corruption::StaticEncodedEmptyInitialState | Corruption::StaticEncodedZeroFillSection
    ) {
        LirStaticInitialState::EncodedStaticValue {
            payload: LirConstantImage::NullPointer(PointerKind::Managed),
        }
    } else {
        LirStaticInitialState::EncodedStaticValue {
            payload: LirConstantImage::GlobalPointer {
                global: immortal,
                kind: PointerKind::Managed,
            },
        }
    };
    globals.alloc(Global {
        address_kind: PointerKind::Raw,
        scan: RefScan::References(vec![0]),
        init: GlobalInit::Storage {
            identity: StaticStorageIdentity::property_backing(
                PropertyOwner::Property(property_id("staticValue")),
                MaterializationRoot::cone_owned(),
            )
            .unwrap(),
            layout: LayoutIdentity::managed_value(
                exact_type("StaticStorage"),
                LirTargetProfile::DARWIN_AARCH64,
                MaterializationRoot::cone_owned(),
            )
            .unwrap()
            .into(),
            ty: LirType::Ptr(PointerKind::Managed),
            initial_state,
            thread_local: false,
        },
    });
    let module = Module {
        cone: ConeIdentity::SINGLE_FILE,
        globals,
        initialization_units: Arena::new(),
        structs: StructDefs::default(),
        enums: EnumDefs::default(),
        functions: vec![function],
        extern_functions: ExternFunctions::default(),
        native_globals: Arena::new(),
        native_global_bridges: NativeGlobalBridges::default(),
        callback_bridges: Arena::new(),
        foreign_callback_families: Arena::new(),
        foreign_callback_bridges: Arena::new(),
        output: scoop_lir::LirOutput::Executable { entry },
        meta: metadata(),
    };
    (module, body, safepoints)
}

fn foundation(
    module: &Module,
    body: &CallableBodyIdentity,
    safepoints: &[SafepointIdentity],
    corruption: Corruption,
) -> (
    OdrFreeLirFoundation,
    Vec<RegistrationArtifacts>,
    CallableRegistrationArtifacts,
    TypeRegistrationArtifacts,
    ImmortalRegistrationArtifacts,
    StaticStorageArtifacts,
) {
    let definition = CborIdentityRecord::from_key(
        ObjectDefinitionPlanKey::strong(
            ConeIdentity::SINGLE_FILE,
            StrongDefinitionEntity::callable_body(body.id()),
            StrongDefinitionRole::CallableBody,
        )
        .unwrap(),
    )
    .unwrap();
    let primary = CborIdentityRecord::from_key(ObjectDefinitionAtomKey::new(
        definition.id(),
        DefinitionAtomRole::Primary,
        DefinitionAtomSubkey::Singleton,
    ))
    .unwrap();
    let stackmap = CborIdentityRecord::from_key(ObjectDefinitionAtomKey::new(
        definition.id(),
        if matches!(corruption, Corruption::WrongStackmapAtomRole) {
            DefinitionAtomRole::AddressTakenConstant
        } else {
            DefinitionAtomRole::Stackmap
        },
        DefinitionAtomSubkey::Singleton,
    ))
    .unwrap();
    let registrations = safepoints
        .iter()
        .map(|safepoint| registration_artifacts(safepoint.site_id()))
        .collect::<Vec<_>>();
    let callable_registration = callable_registration_artifacts(body.id());
    let type_registration = type_registration_artifacts(exact_type("String"));
    let immortal = module
        .globals
        .iter()
        .find_map(|(_, global)| match &global.init {
            GlobalInit::StringConst { identity, .. } => Some(identity.identity_record().clone()),
            GlobalInit::CString { .. }
            | GlobalInit::Storage { .. }
            | GlobalInit::ImportedStorage { .. } => None,
        })
        .unwrap();
    let immortal_registration = immortal_registration_artifacts(immortal);
    let static_storage = static_storage_artifacts(module);
    let local_type_descriptor = module
        .meta
        .type_descriptors
        .iter()
        .map(|(_, descriptor)| descriptor)
        .find(|descriptor| descriptor.identity.exact_type() == type_registration.exact_type)
        .expect("the semantic fixture defines its local String descriptor");
    let mut canonical = CanonicalLirFoundation::empty();
    canonical
        .set_callable_bodies(vec![body.identity_record().clone()])
        .unwrap();
    canonical
        .set_safepoint_sites(
            safepoints
                .iter()
                .map(|identity| identity.site_record().clone())
                .collect(),
        )
        .unwrap();
    canonical
        .set_safepoints(
            safepoints
                .iter()
                .map(SafepointMappingRecord::from_identity)
                .collect(),
        )
        .unwrap();
    canonical
        .set_layouts(vec![
            type_registration.layout.clone(),
            static_storage.layout.clone(),
        ])
        .unwrap();
    canonical
        .set_scans(vec![
            local_type_descriptor.instance_layout.scan_record().clone(),
            static_storage.scan.clone(),
        ])
        .unwrap();
    canonical
        .set_dispatch_tables(vec![local_type_descriptor.vtable.identity_record().clone()])
        .unwrap();
    canonical
        .set_static_storages(vec![static_storage.storage.clone()])
        .unwrap();
    canonical
        .set_immortal_objects(vec![immortal_registration.object.clone()])
        .unwrap();
    canonical
        .set_runtime_types(vec![
            RuntimeTypeMappingRecord::new(type_registration.exact_type).unwrap(),
        ])
        .unwrap();
    canonical
        .set_definition_plans(
            std::iter::once(definition)
                .chain(std::iter::once(callable_registration.plan.clone()))
                .chain([
                    type_registration.descriptor_plan.clone(),
                    type_registration.layout_plan.clone(),
                    type_registration.registration_plan.clone(),
                ])
                .chain([
                    immortal_registration.object_plan.clone(),
                    immortal_registration.registration_plan.clone(),
                ])
                .chain([
                    static_storage.storage_plan.clone(),
                    static_storage.registration_plan.clone(),
                    static_storage.layout_plan.clone(),
                    static_storage.scan_plan.clone(),
                ])
                .chain(
                    registrations
                        .iter()
                        .map(|registration| registration.plan.clone()),
                )
                .collect(),
        )
        .unwrap();
    canonical
        .set_definition_atoms(
            [primary, stackmap]
                .into_iter()
                .chain(std::iter::once(callable_registration.primary.clone()))
                .chain([
                    type_registration.descriptor_primary.clone(),
                    type_registration.descriptor_diagnostic.clone(),
                    type_registration.layout_primary.clone(),
                    type_registration.registration_primary.clone(),
                ])
                .chain([
                    immortal_registration.object_primary.clone(),
                    immortal_registration.registration_primary.clone(),
                ])
                .chain(
                    [
                        Some(static_storage.storage_primary.clone()),
                        static_storage.template_atom.clone(),
                        static_storage.relocation_atom.clone(),
                        Some(static_storage.registration_primary.clone()),
                        Some(static_storage.layout_primary.clone()),
                        Some(static_storage.scan_primary.clone()),
                    ]
                    .into_iter()
                    .flatten(),
                )
                .chain(
                    registrations
                        .iter()
                        .map(|registration| registration.primary.clone()),
                )
                .collect(),
        )
        .unwrap();
    canonical.set_symbol_requests(
        PersistentSymbolRequestTable::new(
            std::iter::once(body.symbol_request())
                .chain(std::iter::once(callable_registration.symbol))
                .chain([
                    type_registration.descriptor_symbol,
                    type_registration.layout_symbol,
                    type_registration.registration_symbol,
                ])
                .chain([
                    immortal_registration.object_symbol,
                    immortal_registration.registration_symbol,
                ])
                .chain([
                    static_storage.storage_symbol,
                    static_storage.registration_symbol,
                    static_storage.layout_symbol,
                    static_storage.scan_symbol,
                ])
                .chain(registrations.iter().map(|registration| registration.symbol))
                .collect(),
        )
        .unwrap(),
    );
    (
        OdrFreeLirFoundation::try_new(ConeIdentity::SINGLE_FILE, canonical).unwrap(),
        registrations,
        callable_registration,
        type_registration,
        immortal_registration,
        static_storage,
    )
}

struct RegistrationArtifacts {
    site: PersistentSafepointSiteId,
    plan: CborIdentityRecord<ObjectDefinitionPlanId, ObjectDefinitionPlanKey>,
    primary: CborIdentityRecord<ObjectDefinitionAtomId, ObjectDefinitionAtomKey>,
    symbol: PersistentSymbolRequest,
}

struct CallableRegistrationArtifacts {
    plan: CborIdentityRecord<ObjectDefinitionPlanId, ObjectDefinitionPlanKey>,
    primary: CborIdentityRecord<ObjectDefinitionAtomId, ObjectDefinitionAtomKey>,
    symbol: PersistentSymbolRequest,
}

struct TypeRegistrationArtifacts {
    exact_type: PersistentExactTypeId,
    layout: CborIdentityRecord<PersistentLayoutId, LayoutKey>,
    descriptor_plan: CborIdentityRecord<ObjectDefinitionPlanId, ObjectDefinitionPlanKey>,
    descriptor_primary: CborIdentityRecord<ObjectDefinitionAtomId, ObjectDefinitionAtomKey>,
    descriptor_diagnostic: CborIdentityRecord<ObjectDefinitionAtomId, ObjectDefinitionAtomKey>,
    descriptor_symbol: PersistentSymbolRequest,
    layout_plan: CborIdentityRecord<ObjectDefinitionPlanId, ObjectDefinitionPlanKey>,
    layout_primary: CborIdentityRecord<ObjectDefinitionAtomId, ObjectDefinitionAtomKey>,
    layout_symbol: PersistentSymbolRequest,
    registration_plan: CborIdentityRecord<ObjectDefinitionPlanId, ObjectDefinitionPlanKey>,
    registration_primary: CborIdentityRecord<ObjectDefinitionAtomId, ObjectDefinitionAtomKey>,
    registration_symbol: PersistentSymbolRequest,
}

struct ImmortalRegistrationArtifacts {
    object: CborIdentityRecord<PersistentImmortalObjectId, ImmortalObjectKey>,
    object_plan: CborIdentityRecord<ObjectDefinitionPlanId, ObjectDefinitionPlanKey>,
    object_primary: CborIdentityRecord<ObjectDefinitionAtomId, ObjectDefinitionAtomKey>,
    object_symbol: PersistentSymbolRequest,
    registration_plan: CborIdentityRecord<ObjectDefinitionPlanId, ObjectDefinitionPlanKey>,
    registration_primary: CborIdentityRecord<ObjectDefinitionAtomId, ObjectDefinitionAtomKey>,
    registration_symbol: PersistentSymbolRequest,
}

struct StaticStorageArtifacts {
    storage: CborIdentityRecord<PersistentStaticStorageId, scoop_identity::StaticStorageKey>,
    layout: CborIdentityRecord<PersistentLayoutId, LayoutKey>,
    scan: CborIdentityRecord<PersistentScanId, ScanKey>,
    storage_plan: CborIdentityRecord<ObjectDefinitionPlanId, ObjectDefinitionPlanKey>,
    storage_primary: CborIdentityRecord<ObjectDefinitionAtomId, ObjectDefinitionAtomKey>,
    template_atom: Option<CborIdentityRecord<ObjectDefinitionAtomId, ObjectDefinitionAtomKey>>,
    relocation_atom: Option<CborIdentityRecord<ObjectDefinitionAtomId, ObjectDefinitionAtomKey>>,
    storage_symbol: PersistentSymbolRequest,
    registration_plan: CborIdentityRecord<ObjectDefinitionPlanId, ObjectDefinitionPlanKey>,
    registration_primary: CborIdentityRecord<ObjectDefinitionAtomId, ObjectDefinitionAtomKey>,
    registration_symbol: PersistentSymbolRequest,
    layout_plan: CborIdentityRecord<ObjectDefinitionPlanId, ObjectDefinitionPlanKey>,
    layout_primary: CborIdentityRecord<ObjectDefinitionAtomId, ObjectDefinitionAtomKey>,
    layout_symbol: PersistentSymbolRequest,
    scan_plan: CborIdentityRecord<ObjectDefinitionPlanId, ObjectDefinitionPlanKey>,
    scan_primary: CborIdentityRecord<ObjectDefinitionAtomId, ObjectDefinitionAtomKey>,
    scan_symbol: PersistentSymbolRequest,
}

fn static_storage_artifacts(module: &Module) -> StaticStorageArtifacts {
    let (identity, layout, initial_state) = module
        .globals
        .iter()
        .find_map(|(_, global)| match &global.init {
            GlobalInit::Storage {
                identity,
                layout,
                initial_state,
                ..
            } => Some((identity, layout, initial_state)),
            GlobalInit::CString { .. }
            | GlobalInit::StringConst { .. }
            | GlobalInit::ImportedStorage { .. } => None,
        })
        .unwrap();
    let storage = identity.identity_record().clone();
    let layout = layout.local().unwrap();
    let layout_record = layout.layout_record().clone();
    let scan = layout.scan_record().clone();
    let storage_plan = strong_definition(
        StrongDefinitionEntity::static_storage(storage.id()),
        StrongDefinitionRole::StaticStorage,
    );
    let registration_plan = strong_definition(
        StrongDefinitionEntity::static_storage(storage.id()),
        StrongDefinitionRole::RootRegistration,
    );
    let layout_plan = strong_definition(
        StrongDefinitionEntity::layout(layout_record.id()),
        StrongDefinitionRole::Layout,
    );
    let scan_plan = strong_definition(
        StrongDefinitionEntity::scan(scan.id()),
        StrongDefinitionRole::ScanProgram,
    );
    let template_atom = matches!(
        initial_state,
        LirStaticInitialState::EncodedStaticValue { .. }
    )
    .then(|| {
        CborIdentityRecord::from_key(ObjectDefinitionAtomKey::new(
            storage_plan.id(),
            DefinitionAtomRole::AddressTakenConstant,
            DefinitionAtomSubkey::StaticStorage(storage.id()),
        ))
        .unwrap()
    });
    let relocation_atom = matches!(
        initial_state,
        LirStaticInitialState::EncodedStaticValue {
            payload: LirConstantImage::GlobalPointer { .. }
        }
    )
    .then(|| {
        CborIdentityRecord::from_key(ObjectDefinitionAtomKey::new(
            storage_plan.id(),
            DefinitionAtomRole::RuntimeRecord,
            DefinitionAtomSubkey::StaticStorage(storage.id()),
        ))
        .unwrap()
    });
    StaticStorageArtifacts {
        storage_symbol: identity.symbol_request(),
        registration_symbol: strong_symbol(PersistentSymbolKey::RootRegistration(storage.id())),
        layout_symbol: strong_symbol(PersistentSymbolKey::Layout(layout_record.id())),
        scan_symbol: strong_symbol(PersistentSymbolKey::ScanProgram(scan.id())),
        storage_primary: primary_atom(&storage_plan),
        template_atom,
        relocation_atom,
        registration_primary: primary_atom(&registration_plan),
        layout_primary: primary_atom(&layout_plan),
        scan_primary: primary_atom(&scan_plan),
        storage,
        layout: layout_record,
        scan,
        storage_plan,
        registration_plan,
        layout_plan,
        scan_plan,
    }
}

fn immortal_registration_artifacts(
    object: CborIdentityRecord<PersistentImmortalObjectId, ImmortalObjectKey>,
) -> ImmortalRegistrationArtifacts {
    let object_plan = strong_definition(
        StrongDefinitionEntity::immortal_object(object.id()),
        StrongDefinitionRole::ImmortalObject,
    );
    let registration_plan = strong_definition(
        StrongDefinitionEntity::immortal_object(object.id()),
        StrongDefinitionRole::ImmortalRegistration,
    );
    ImmortalRegistrationArtifacts {
        object_symbol: strong_symbol(PersistentSymbolKey::ImmortalObject(object.id())),
        registration_symbol: strong_symbol(PersistentSymbolKey::ImmortalRegistration(object.id())),
        object_primary: primary_atom(&object_plan),
        registration_primary: primary_atom(&registration_plan),
        object,
        object_plan,
        registration_plan,
    }
}

fn type_registration_artifacts(exact_type: PersistentExactTypeId) -> TypeRegistrationArtifacts {
    let layout = CborIdentityRecord::from_key(LayoutKey::darwin_aarch64(
        exact_type,
        RepresentationRole::ManagedObject,
    ))
    .unwrap();
    let descriptor_plan = strong_definition(
        StrongDefinitionEntity::exact_type(exact_type),
        StrongDefinitionRole::TypeDescriptor,
    );
    let layout_plan = strong_definition(
        StrongDefinitionEntity::layout(layout.id()),
        StrongDefinitionRole::Layout,
    );
    let registration_plan = strong_definition(
        StrongDefinitionEntity::exact_type(exact_type),
        StrongDefinitionRole::TypeRegistration,
    );
    let layout_symbol = strong_symbol(PersistentSymbolKey::Layout(layout.id()));
    TypeRegistrationArtifacts {
        exact_type,
        layout,
        descriptor_primary: primary_atom(&descriptor_plan),
        descriptor_diagnostic: CborIdentityRecord::from_key(ObjectDefinitionAtomKey::new(
            descriptor_plan.id(),
            DefinitionAtomRole::AddressTakenConstant,
            DefinitionAtomSubkey::ExactType(exact_type),
        ))
        .unwrap(),
        descriptor_symbol: strong_symbol(PersistentSymbolKey::TypeDescriptor(exact_type)),
        layout_primary: primary_atom(&layout_plan),
        layout_symbol,
        registration_primary: primary_atom(&registration_plan),
        registration_symbol: strong_symbol(PersistentSymbolKey::TypeRegistration(exact_type)),
        descriptor_plan,
        layout_plan,
        registration_plan,
    }
}

fn strong_definition(
    entity: StrongDefinitionEntity,
    role: StrongDefinitionRole,
) -> CborIdentityRecord<ObjectDefinitionPlanId, ObjectDefinitionPlanKey> {
    CborIdentityRecord::from_key(
        ObjectDefinitionPlanKey::strong(ConeIdentity::SINGLE_FILE, entity, role).unwrap(),
    )
    .unwrap()
}

fn primary_atom(
    plan: &CborIdentityRecord<ObjectDefinitionPlanId, ObjectDefinitionPlanKey>,
) -> CborIdentityRecord<ObjectDefinitionAtomId, ObjectDefinitionAtomKey> {
    CborIdentityRecord::from_key(ObjectDefinitionAtomKey::new(
        plan.id(),
        DefinitionAtomRole::Primary,
        DefinitionAtomSubkey::Singleton,
    ))
    .unwrap()
}

fn strong_symbol(key: PersistentSymbolKey) -> PersistentSymbolRequest {
    PersistentSymbolRequest::new(key, LinkageClass::ConeStrong).unwrap()
}

fn callable_registration_artifacts(
    body: scoop_identity::PersistentCallableBodyId,
) -> CallableRegistrationArtifacts {
    let plan = CborIdentityRecord::from_key(
        ObjectDefinitionPlanKey::strong(
            ConeIdentity::SINGLE_FILE,
            StrongDefinitionEntity::callable_body(body),
            StrongDefinitionRole::CallableRegistration,
        )
        .unwrap(),
    )
    .unwrap();
    let primary = CborIdentityRecord::from_key(ObjectDefinitionAtomKey::new(
        plan.id(),
        DefinitionAtomRole::Primary,
        DefinitionAtomSubkey::Singleton,
    ))
    .unwrap();
    let symbol = PersistentSymbolRequest::new(
        PersistentSymbolKey::CallableRegistration(body),
        LinkageClass::ConeStrong,
    )
    .unwrap();
    CallableRegistrationArtifacts {
        plan,
        primary,
        symbol,
    }
}

fn registration_artifacts(site: PersistentSafepointSiteId) -> RegistrationArtifacts {
    let plan = CborIdentityRecord::from_key(
        ObjectDefinitionPlanKey::strong(
            ConeIdentity::SINGLE_FILE,
            StrongDefinitionEntity::safepoint_site(site),
            StrongDefinitionRole::SafepointRegistration,
        )
        .unwrap(),
    )
    .unwrap();
    let primary = CborIdentityRecord::from_key(ObjectDefinitionAtomKey::new(
        plan.id(),
        DefinitionAtomRole::Primary,
        DefinitionAtomSubkey::Singleton,
    ))
    .unwrap();
    let symbol = PersistentSymbolRequest::new(
        PersistentSymbolKey::SafepointRegistration(site),
        LinkageClass::ConeStrong,
    )
    .unwrap();
    RegistrationArtifacts {
        site,
        plan,
        primary,
        symbol,
    }
}

fn digest_plan(
    foundation: &OdrFreeLirFoundation,
    body: scoop_identity::PersistentCallableBodyId,
    registrations: &[RegistrationArtifacts],
    callable_registration: &CallableRegistrationArtifacts,
    type_registration: &TypeRegistrationArtifacts,
    immortal_registration: &ImmortalRegistrationArtifacts,
    static_storage: &StaticStorageArtifacts,
) -> StrongDigestFinalizationPlanV1 {
    let mut nodes = Vec::new();
    let mut image_inputs = Vec::new();
    let mut body_inputs = Vec::new();
    for registration in registrations {
        let object = DigestNodeV1::new(
            DigestNodeKey::object_definition(registration.primary.id()),
            Vec::new(),
            Vec::new(),
        )
        .unwrap();
        let stackmap_key = DigestNodeKey::stackmap_record(registration.site);
        let stackmap_source = DigestNodeId::from_key(&stackmap_key).unwrap();
        let stackmap = DigestNodeV1::new(
            stackmap_key,
            Vec::new(),
            vec![DigestPatchIntentKey::new(
                stackmap_source,
                registration.plan.id(),
                DefinitionAtomRole::Primary,
                DigestSemanticFieldRole::NormalizedStackmap,
            )],
        )
        .unwrap();
        body_inputs.push(DigestInputRefV1::from_node(&stackmap));
        let registration_key = DigestNodeKey::strong_registration(registration.plan.id());
        let registration_source = DigestNodeId::from_key(&registration_key).unwrap();
        let fingerprint = DigestNodeV1::new(
            registration_key,
            vec![
                DigestInputRefV1::from_node(&object),
                DigestInputRefV1::from_node(&stackmap),
            ],
            vec![DigestPatchIntentKey::new(
                registration_source,
                registration.plan.id(),
                DefinitionAtomRole::Primary,
                DigestSemanticFieldRole::RegistrationDefinition,
            )],
        )
        .unwrap();
        image_inputs.push(DigestInputRefV1::from_node(&fingerprint));
        nodes.extend([object, stackmap, fingerprint]);
    }
    let body_definition = definition_plan(body);
    let body_primary = ObjectDefinitionAtomId::from_key(&ObjectDefinitionAtomKey::new(
        body_definition,
        DefinitionAtomRole::Primary,
        DefinitionAtomSubkey::Singleton,
    ))
    .unwrap();
    let body_key = DigestNodeKey::object_definition(body_primary);
    let body_source = DigestNodeId::from_key(&body_key).unwrap();
    let body_node = DigestNodeV1::new(
        body_key,
        body_inputs,
        vec![DigestPatchIntentKey::new(
            body_source,
            callable_registration.plan.id(),
            DefinitionAtomRole::Primary,
            DigestSemanticFieldRole::CallableBodyDefinition,
        )],
    )
    .unwrap();
    let callable_object = DigestNodeV1::new(
        DigestNodeKey::object_definition(callable_registration.primary.id()),
        Vec::new(),
        Vec::new(),
    )
    .unwrap();
    let callable_key = DigestNodeKey::strong_registration(callable_registration.plan.id());
    let callable_source = DigestNodeId::from_key(&callable_key).unwrap();
    let callable_fingerprint = DigestNodeV1::new(
        callable_key,
        vec![
            DigestInputRefV1::from_node(&body_node),
            DigestInputRefV1::from_node(&callable_object),
        ],
        vec![DigestPatchIntentKey::new(
            callable_source,
            callable_registration.plan.id(),
            DefinitionAtomRole::Primary,
            DigestSemanticFieldRole::RegistrationDefinition,
        )],
    )
    .unwrap();
    image_inputs.push(DigestInputRefV1::from_node(&callable_fingerprint));
    nodes.extend([body_node, callable_object, callable_fingerprint]);

    let descriptor_key =
        DigestNodeKey::object_definition(type_registration.descriptor_primary.id());
    let descriptor_source = DigestNodeId::from_key(&descriptor_key).unwrap();
    let descriptor = DigestNodeV1::new(
        descriptor_key,
        Vec::new(),
        vec![DigestPatchIntentKey::new(
            descriptor_source,
            type_registration.registration_plan.id(),
            DefinitionAtomRole::Primary,
            DigestSemanticFieldRole::DescriptorDefinition,
        )],
    )
    .unwrap();
    let layout_key = DigestNodeKey::layout(type_registration.layout.id());
    let layout_source = DigestNodeId::from_key(&layout_key).unwrap();
    let layout = DigestNodeV1::new(
        layout_key,
        Vec::new(),
        vec![DigestPatchIntentKey::new(
            layout_source,
            type_registration.registration_plan.id(),
            DefinitionAtomRole::Primary,
            DigestSemanticFieldRole::Layout,
        )],
    )
    .unwrap();
    let type_object = DigestNodeV1::new(
        DigestNodeKey::object_definition(type_registration.registration_primary.id()),
        Vec::new(),
        Vec::new(),
    )
    .unwrap();
    let type_key = DigestNodeKey::strong_registration(type_registration.registration_plan.id());
    let type_source = DigestNodeId::from_key(&type_key).unwrap();
    let type_fingerprint = DigestNodeV1::new(
        type_key,
        vec![
            DigestInputRefV1::from_node(&type_object),
            DigestInputRefV1::from_node(&descriptor),
            DigestInputRefV1::from_node(&layout),
        ],
        vec![DigestPatchIntentKey::new(
            type_source,
            type_registration.registration_plan.id(),
            DefinitionAtomRole::Primary,
            DigestSemanticFieldRole::RegistrationDefinition,
        )],
    )
    .unwrap();
    image_inputs.push(DigestInputRefV1::from_node(&type_fingerprint));
    nodes.extend([descriptor, layout, type_object, type_fingerprint]);

    let immortal_object = DigestNodeV1::new(
        DigestNodeKey::object_definition(immortal_registration.object_primary.id()),
        Vec::new(),
        Vec::new(),
    )
    .unwrap();
    let immortal_registration_object = DigestNodeV1::new(
        DigestNodeKey::object_definition(immortal_registration.registration_primary.id()),
        Vec::new(),
        Vec::new(),
    )
    .unwrap();
    let immortal_key =
        DigestNodeKey::strong_registration(immortal_registration.registration_plan.id());
    let immortal_source = DigestNodeId::from_key(&immortal_key).unwrap();
    let immortal_fingerprint = DigestNodeV1::new(
        immortal_key,
        vec![
            DigestInputRefV1::from_node(&immortal_registration_object),
            DigestInputRefV1::from_node(&immortal_object),
        ],
        vec![DigestPatchIntentKey::new(
            immortal_source,
            immortal_registration.registration_plan.id(),
            DefinitionAtomRole::Primary,
            DigestSemanticFieldRole::RegistrationDefinition,
        )],
    )
    .unwrap();
    image_inputs.push(DigestInputRefV1::from_node(&immortal_fingerprint));
    nodes.extend([
        immortal_object,
        immortal_registration_object,
        immortal_fingerprint,
    ]);

    let static_storage_object = DigestNodeV1::new(
        DigestNodeKey::object_definition(static_storage.storage_primary.id()),
        Vec::new(),
        Vec::new(),
    )
    .unwrap();
    let static_registration_object = DigestNodeV1::new(
        DigestNodeKey::object_definition(static_storage.registration_primary.id()),
        Vec::new(),
        Vec::new(),
    )
    .unwrap();
    let static_layout_key = DigestNodeKey::layout(static_storage.layout.id());
    let static_layout_source = DigestNodeId::from_key(&static_layout_key).unwrap();
    let static_layout = DigestNodeV1::new(
        static_layout_key,
        Vec::new(),
        vec![DigestPatchIntentKey::new(
            static_layout_source,
            static_storage.registration_plan.id(),
            DefinitionAtomRole::Primary,
            DigestSemanticFieldRole::Layout,
        )],
    )
    .unwrap();
    let static_scan_key = DigestNodeKey::scan(static_storage.scan.id());
    let static_scan_source = DigestNodeId::from_key(&static_scan_key).unwrap();
    let static_scan = DigestNodeV1::new(
        static_scan_key,
        Vec::new(),
        vec![DigestPatchIntentKey::new(
            static_scan_source,
            static_storage.registration_plan.id(),
            DefinitionAtomRole::Primary,
            DigestSemanticFieldRole::Scan,
        )],
    )
    .unwrap();
    let static_key = DigestNodeKey::strong_registration(static_storage.registration_plan.id());
    let static_source = DigestNodeId::from_key(&static_key).unwrap();
    let static_fingerprint = DigestNodeV1::new(
        static_key,
        vec![
            DigestInputRefV1::from_node(&static_registration_object),
            DigestInputRefV1::from_node(&static_storage_object),
            DigestInputRefV1::from_node(&static_layout),
            DigestInputRefV1::from_node(&static_scan),
        ],
        vec![DigestPatchIntentKey::new(
            static_source,
            static_storage.registration_plan.id(),
            DefinitionAtomRole::Primary,
            DigestSemanticFieldRole::RegistrationDefinition,
        )],
    )
    .unwrap();
    image_inputs.push(DigestInputRefV1::from_node(&static_fingerprint));
    nodes.extend([
        static_storage_object,
        static_registration_object,
        static_layout,
        static_scan,
        static_fingerprint,
    ]);
    nodes.push(
        DigestNodeV1::new(
            DigestNodeKey::runtime_image(foundation.producer()),
            image_inputs,
            Vec::new(),
        )
        .unwrap(),
    );
    StrongDigestFinalizationPlanV1::new(nodes, foundation).unwrap()
}

fn definition_plan(owner: scoop_identity::PersistentCallableBodyId) -> ObjectDefinitionPlanId {
    let key = ObjectDefinitionPlanKey::strong(
        ConeIdentity::SINGLE_FILE,
        StrongDefinitionEntity::callable_body(owner),
        StrongDefinitionRole::CallableBody,
    )
    .unwrap();
    ObjectDefinitionPlanId::from_key(&key).unwrap()
}

fn metadata() -> LirMeta {
    let string_type = exact_type("String");
    let runtime_type = RuntimeTypeMappingRecord::new(string_type).unwrap();
    let mut layouts = Arena::new();
    let string_layout = layouts.alloc(Layout {
        identity: LayoutIdentity::managed_object(
            string_type,
            LirTargetProfile::DARWIN_AARCH64,
            MaterializationRoot::cone_owned(),
        )
        .unwrap(),
        name: "String".to_string(),
        size: 24,
        align: 8,
        fields: Vec::new(),
        c_layout: None,
        interior_mutable: false,
        kind: LayoutKind::Intrinsic(IntrinsicTypeRepresentation::String),
    });
    let mut type_descriptors = Arena::new();
    let identity =
        TypeDescriptorIdentity::new(runtime_type, MaterializationRoot::cone_owned()).unwrap();
    let vtable = VtableRecord::new(&identity, Vec::new()).unwrap();
    let string_descriptor = type_descriptors.alloc(TypeDescriptor {
        diagnostic_name: "String".to_string(),
        identity,
        instance_layout: layouts[string_layout].identity.clone(),
        instance_shape: TypeInstanceShapeV1::inline_bytes(LirTargetProfile::DARWIN_AARCH64)
            .unwrap(),
        inline_scan: scoop_lir::TypeDescriptorInlineScanV1::Null,
        parent: None,
        vtable,
        itables: Vec::new(),
    });
    let external_type_descriptors = Arena::new();
    let well_known_string = TypeDescriptorRef::Local(string_descriptor);
    LirMeta {
        exact_types: Vec::new(),
        target_profile: LirTargetProfile::DARWIN_AARCH64,
        canonical_c_abi: CanonicalCAbiMetadata::default(),
        native_externals: NativeExternalMetadata::default(),
        well_known_type_descriptors: WellKnownTypeDescriptors {
            string: well_known_string,
        },
        arrays: Arena::new(),
        layouts,
        type_descriptors,
        external_type_descriptors,
        external_callables: Arena::new(),
    }
}

fn function_id(name: &str) -> PersistentFunctionId {
    PersistentFunctionId::from_source_declaration(&SourceDeclarationKey::function(
        source_site(),
        CanonicalIdentifier::new(name).unwrap(),
        0,
        None,
        Vec::new(),
    ))
    .unwrap()
}

fn exact_type(name: &str) -> PersistentExactTypeId {
    let nominal = PersistentTypeId::from_source_declaration(&SourceDeclarationKey::nominal(
        source_site(),
        CanonicalIdentifier::new(name).unwrap(),
        SourceNominalKind::Class,
        0,
    ))
    .unwrap();
    PersistentExactTypeId::from_key(&ExactTypeKey::Nominal(nominal)).unwrap()
}

fn immortal_key() -> ImmortalObjectKey {
    let property = property_id("text");
    ImmortalObjectKey::string_constant(
        ImmortalObjectOwner::Property(PropertyOwner::Property(property)),
        StructuralDefinitionPath::from_first(
            StructuralPathSegment::new(StructuralDefinitionSiteRole::StringConstant, 0),
            [],
        ),
    )
}

fn property_id(name: &str) -> PersistentPropertyId {
    PersistentPropertyId::from_source_declaration(&SourceDeclarationKey::property(
        source_site(),
        CanonicalIdentifier::new(name).unwrap(),
    ))
    .unwrap()
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
