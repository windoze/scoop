use la_arena::Arena;
use scoop_identity::{
    CanonicalIdentifier, CborIdentityRecord, ConeIdentity, DeclarationScope, DefinitionAtomRole,
    DefinitionAtomSubkey, DefinitionOwnerChain, DigestNodeId, DigestNodeKey, DigestPatchIntentKey,
    DigestSemanticFieldRole, ExactTypeKey, LinkageClass, ObjectDefinitionAtomKey,
    ObjectDefinitionPlanKey, PackagePath, PersistentExactTypeId, PersistentFunctionId,
    PersistentSymbolKey, PersistentSymbolRequest, PersistentSymbolRequestTable, PersistentTypeId,
    SourceDeclarationKey, SourceDeclarationSite, SourceNominalKind, StrongDefinitionEntity,
    StrongDefinitionRole,
};
use scoop_lir::{
    AbiReturn, BasicBlock, CallTarget, CallTargets, CallableBodyIdentity, CanonicalCAbiMetadata,
    CanonicalLirFoundation, DigestInputRefV1, DigestNodeV1, EnumDefs, ExternFunctions, Function,
    GcEffect, Instruction, IntrinsicTypeRepresentation, Layout, LayoutIdentity, LayoutKind,
    LirMeta, LirTargetProfile, LocalFunctionIdentities, LocalFunctionRef, ManagedCallDestination,
    ManagedLeafPath, ManagedLeafPaths, ManagedPollSite, ManagedRuntimeFunction,
    MaterializationRoot, Module, NativeExternalMetadata, NativeGlobalBridges, OdrFreeLirFoundation,
    RuntimeTypeMappingRecord, SafepointIdentities, SafepointIdentity, SafepointMappingRecord,
    SafepointSiteRef, SafepointSiteRole, ScoopAbiSignature, StatepointLiveSet, StatepointLiveValue,
    StrongDigestFinalizationPlanV1, StrongRegistrationIdentitySurfaceV1,
    StrongSafepointRegistrationPlanSetV1, StrongSafepointSemanticPlanSetV1, StructDefs, Terminator,
    TypeDescriptor, TypeDescriptorIdentity, TypeDescriptorRef, TypeInstanceShapeV1,
    VoidCallSignature, VtableRecord, WellKnownTypeDescriptors,
};

pub(super) fn safepoint_plan() -> StrongSafepointRegistrationPlanSetV1 {
    let (module, body, safepoint) = semantic_module();
    let semantics = StrongSafepointSemanticPlanSetV1::from_module(&module).unwrap();
    let plan = CborIdentityRecord::from_key(
        ObjectDefinitionPlanKey::strong(
            ConeIdentity::SINGLE_FILE,
            StrongDefinitionEntity::safepoint_site(safepoint.site_id()),
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
        PersistentSymbolKey::SafepointRegistration(safepoint.site_id()),
        LinkageClass::ConeStrong,
    )
    .unwrap();
    let mut canonical = CanonicalLirFoundation::empty();
    canonical
        .set_callable_bodies(vec![body.identity_record().clone()])
        .unwrap();
    canonical
        .set_safepoint_sites(vec![safepoint.site_record().clone()])
        .unwrap();
    canonical
        .set_safepoints(vec![SafepointMappingRecord::from_identity(&safepoint)])
        .unwrap();
    canonical.set_definition_plans(vec![plan.clone()]).unwrap();
    canonical
        .set_definition_atoms(vec![primary.clone()])
        .unwrap();
    canonical.set_symbol_requests(PersistentSymbolRequestTable::new(vec![symbol]).unwrap());
    let foundation = OdrFreeLirFoundation::try_new(ConeIdentity::SINGLE_FILE, canonical).unwrap();

    let object = DigestNodeV1::new(
        DigestNodeKey::object_definition(primary.id()),
        Vec::new(),
        Vec::new(),
    )
    .unwrap();
    let stackmap_key = DigestNodeKey::stackmap_record(safepoint.site_id());
    let stackmap_id = DigestNodeId::from_key(&stackmap_key).unwrap();
    let stackmap = DigestNodeV1::new(
        stackmap_key,
        Vec::new(),
        vec![DigestPatchIntentKey::new(
            stackmap_id,
            plan.id(),
            DefinitionAtomRole::Primary,
            DigestSemanticFieldRole::NormalizedStackmap,
        )],
    )
    .unwrap();
    let registration_key = DigestNodeKey::strong_registration(plan.id());
    let registration_id = DigestNodeId::from_key(&registration_key).unwrap();
    let registration = DigestNodeV1::new(
        registration_key,
        vec![
            DigestInputRefV1::from_node(&object),
            DigestInputRefV1::from_node(&stackmap),
        ],
        vec![DigestPatchIntentKey::new(
            registration_id,
            plan.id(),
            DefinitionAtomRole::Primary,
            DigestSemanticFieldRole::RegistrationDefinition,
        )],
    )
    .unwrap();
    let image = DigestNodeV1::new(
        DigestNodeKey::runtime_image(ConeIdentity::SINGLE_FILE),
        vec![DigestInputRefV1::from_node(&registration)],
        Vec::new(),
    )
    .unwrap();
    let digests = StrongDigestFinalizationPlanV1::new(
        vec![object, stackmap, registration, image],
        &foundation,
    )
    .unwrap();
    let identities =
        StrongRegistrationIdentitySurfaceV1::from_foundation(&foundation, &digests).unwrap();
    StrongSafepointRegistrationPlanSetV1::new(&foundation, &identities, &semantics, &digests)
        .unwrap()
}

fn semantic_module() -> (Module, CallableBodyIdentity, SafepointIdentity) {
    let body = CallableBodyIdentity::for_function(function_id("codegenSafepoint")).unwrap();
    let safepoint = SafepointIdentity::new(body.id(), SafepointSiteRole::ManagedPoll, 0).unwrap();
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
        instructions: vec![Instruction::ManagedPoll {
            site: ManagedPollSite {
                target,
                safepoint: SafepointSiteRef::from_u32(0),
                live: live_roots(),
            },
        }],
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
        safepoints: SafepointIdentities::checked(vec![(
            SafepointSiteRef::from_u32(0),
            safepoint.clone(),
        )])
        .unwrap(),
        locals: Arena::new(),
        temps: Arena::new(),
        blocks,
        entry,
    };
    let mut local_functions = LocalFunctionIdentities::default();
    let entry = LocalFunctionRef::Managed(local_functions.alloc_managed());
    let module = Module {
        cone: ConeIdentity::SINGLE_FILE,
        globals: Arena::new(),
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
    (module, body, safepoint)
}

fn live_roots() -> StatepointLiveSet {
    StatepointLiveSet::new(vec![StatepointLiveValue {
        source: scoop_lir::CallerRootSource::Param(0),
        ty: scoop_lir::LirType::Aggregate(vec![scoop_lir::MANAGED_PTR; 2]),
        leaves: ManagedLeafPaths::new(vec![
            ManagedLeafPath { byte_offset: 0 },
            ManagedLeafPath { byte_offset: 8 },
        ])
        .unwrap(),
    }])
    .unwrap()
}

fn metadata() -> LirMeta {
    let exact_type = exact_type("String");
    let runtime_type = RuntimeTypeMappingRecord::new(exact_type).unwrap();
    let mut layouts = Arena::new();
    let string_layout = layouts.alloc(Layout {
        identity: LayoutIdentity::managed_object(
            exact_type,
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
    LirMeta {
        exact_types: Vec::new(),
        target_profile: LirTargetProfile::DARWIN_AARCH64,
        canonical_c_abi: CanonicalCAbiMetadata::default(),
        native_externals: NativeExternalMetadata::default(),
        well_known_type_descriptors: WellKnownTypeDescriptors {
            string: TypeDescriptorRef::Local(string_descriptor),
        },
        arrays: Arena::new(),
        layouts,
        type_descriptors,
        core_external_type_descriptors: Arena::new(),
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

fn source_site() -> SourceDeclarationSite {
    SourceDeclarationSite::new(
        ConeIdentity::SINGLE_FILE,
        PackagePath::root(),
        DefinitionOwnerChain::top_level(),
        DeclarationScope::ConeWide,
    )
    .unwrap()
}
