use la_arena::Arena;
use scoop_identity::{
    CanonicalIdentifier, CborIdentityRecord, ConeIdentity, DeclarationScope, DefinitionAtomRole,
    DefinitionAtomSubkey, DefinitionOwnerChain, DigestNodeId, DigestNodeKey, DigestPatchIntentKey,
    DigestSemanticFieldRole, ExactTypeKey, ImmortalObjectKey, ImmortalObjectOwner, LayoutKey,
    LinkageClass, ObjectDefinitionAtomId, ObjectDefinitionAtomKey, ObjectDefinitionPlanId,
    ObjectDefinitionPlanKey, PackagePath, PersistentExactTypeId, PersistentFunctionId,
    PersistentImmortalObjectId, PersistentLayoutId, PersistentPropertyId,
    PersistentSafepointSiteId, PersistentSymbolKey, PersistentSymbolRequest,
    PersistentSymbolRequestTable, PersistentTypeId, PropertyOwner, RepresentationRole,
    SourceDeclarationKey, SourceDeclarationSite, SourceNominalKind, StrongDefinitionEntity,
    StrongDefinitionRole, StructuralDefinitionPath, StructuralDefinitionSiteRole,
    StructuralPathSegment,
};
use scoop_lir::{
    AbiReturn, BasicBlock, CallTarget, CallTargets, CallableBodyIdentity, CanonicalCAbiMetadata,
    CanonicalLirFoundation, CoreExternalTypeDescriptor, DigestInputRefV1, DigestNodeV1, EnumDefs,
    ExternFunctions, Function, GcEffect, Global, GlobalInit, ImmortalObjectIdentity, Instruction,
    IntrinsicTypeRepresentation, Layout, LayoutIdentity, LayoutKind, LirMeta, LirTargetProfile,
    LocalFunctionIdentities, LocalFunctionRef, ManagedCallDestination, ManagedPollSite,
    ManagedRuntimeFunction, MaterializationRoot, Module, NativeExternalMetadata,
    NativeGlobalBridges, OdrFreeLirFoundation, PointerKind, RefScan, RuntimeTypeMappingRecord,
    SafepointIdentities, SafepointIdentity, SafepointMappingRecord, SafepointSiteRef,
    SafepointSiteRole, ScoopAbiSignature, StatepointLiveSet, StrongCallableRegistrationPlanSetV1,
    StrongDigestFinalizationPlanV1, StrongImmortalObjectRegistrationPlanSetV1,
    StrongImmortalObjectSemanticPlanSetV1, StrongRegistrationIdentitySurfaceV1,
    StrongSafepointRegistrationPlanSetV1, StrongSafepointSemanticPlanSetV1,
    StrongTypeRegistrationPlanSetV1, StructDefs, Terminator, TypeDescriptor,
    TypeDescriptorIdentity, TypeDescriptorRef, TypeDescriptorScan, VoidCallSignature, VtableRecord,
    WellKnownLayouts, WellKnownTypeDescriptors,
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
    pub(crate) safepoint_ids: Vec<u64>,
}

pub(crate) fn inputs(corruption: Corruption) -> SemanticInputs {
    let (module, body, safepoints) = semantic_module(corruption);
    let semantics = StrongSafepointSemanticPlanSetV1::from_module(&module).unwrap();
    let immortal_semantics = StrongImmortalObjectSemanticPlanSetV1::from_module(&module).unwrap();
    let (
        foundation,
        registrations,
        callable_registration,
        type_registration,
        immortal_registration,
    ) = foundation(&module, &body, &safepoints, corruption);
    let digest_plan = digest_plan(
        &foundation,
        body.id(),
        &registrations,
        &callable_registration,
        &type_registration,
        &immortal_registration,
    );
    let identities =
        StrongRegistrationIdentitySurfaceV1::from_foundation(&foundation, &digest_plan).unwrap();
    let registration_plan = StrongSafepointRegistrationPlanSetV1::new(
        &foundation,
        &identities,
        &semantics,
        &digest_plan,
    )
    .unwrap();
    let immortal_registration_plan = StrongImmortalObjectRegistrationPlanSetV1::new(
        &foundation,
        &identities,
        &immortal_semantics,
        &digest_plan,
    )
    .unwrap();
    let callable_registration_plan =
        StrongCallableRegistrationPlanSetV1::new(&foundation, &identities, &digest_plan).unwrap();
    let type_registration_plan = StrongTypeRegistrationPlanSetV1::new(
        LirTargetProfile::DARWIN_AARCH64,
        &foundation,
        &identities,
        &digest_plan,
    )
    .unwrap();
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
        safepoint_ids,
    }
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
    globals.alloc(Global {
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
        entry,
        meta: metadata(corruption),
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
            GlobalInit::CString { .. } | GlobalInit::Storage { .. } => None,
        })
        .unwrap();
    let immortal_registration = immortal_registration_artifacts(immortal);
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
        .set_layouts(vec![type_registration.layout.clone()])
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
                    type_registration.layout_primary.clone(),
                    type_registration.registration_primary.clone(),
                ])
                .chain([
                    immortal_registration.object_primary.clone(),
                    immortal_registration.registration_primary.clone(),
                ])
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

fn metadata(corruption: Corruption) -> LirMeta {
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
    let identity = TypeDescriptorIdentity::runtime_core_string(runtime_type);
    let vtable = VtableRecord::new(&identity, Vec::new()).unwrap();
    let string_descriptor = type_descriptors.alloc(TypeDescriptor {
        name: "String".to_string(),
        identity,
        size: 24,
        align: 8,
        scan: TypeDescriptorScan::Fixed(RefScan::None),
        parent: None,
        vtable,
        itables: Vec::new(),
    });
    let mut core_external_type_descriptors = Arena::new();
    let well_known_string =
        if matches!(corruption, Corruption::CoreExternalImmortalTypeRegistration) {
            TypeDescriptorRef::CoreExternal(
                core_external_type_descriptors
                    .alloc(CoreExternalTypeDescriptor::new(exact_type("CoreString")).unwrap()),
            )
        } else {
            TypeDescriptorRef::Local(string_descriptor)
        };
    LirMeta {
        target_profile: LirTargetProfile::DARWIN_AARCH64,
        canonical_c_abi: CanonicalCAbiMetadata::default(),
        native_externals: NativeExternalMetadata::default(),
        well_known_layouts: WellKnownLayouts {
            string: string_layout,
        },
        well_known_type_descriptors: WellKnownTypeDescriptors {
            string: well_known_string,
        },
        arrays: Arena::new(),
        layouts,
        type_descriptors,
        core_external_type_descriptors,
        core_external_callables: Arena::new(),
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
    let property = PersistentPropertyId::from_source_declaration(&SourceDeclarationKey::property(
        source_site(),
        CanonicalIdentifier::new("text").unwrap(),
    ))
    .unwrap();
    ImmortalObjectKey::string_constant(
        ImmortalObjectOwner::Property(PropertyOwner::Property(property)),
        StructuralDefinitionPath::from_first(
            StructuralPathSegment::new(StructuralDefinitionSiteRole::StringConstant, 0),
            [],
        ),
    )
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
