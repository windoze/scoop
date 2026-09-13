use la_arena::Arena;
use scoop_identity::{
    CanonicalIdentifier, CborIdentityRecord, ConeIdentity, DeclarationScope, DefinitionAtomRole,
    DefinitionAtomSubkey, DefinitionOwnerChain, ExactTypeKey, ObjectDefinitionAtomKey,
    ObjectDefinitionPlanId, ObjectDefinitionPlanKey, PackagePath, PersistentExactTypeId,
    PersistentFunctionId, PersistentSymbolRequestTable, PersistentTypeId, SourceDeclarationKey,
    SourceDeclarationSite, SourceNominalKind, StrongDefinitionEntity, StrongDefinitionRole,
};
use scoop_lir::{
    AbiReturn, BasicBlock, CallTarget, CallTargets, CallableBodyIdentity, CanonicalCAbiMetadata,
    CanonicalLirFoundation, EnumDefs, ExternFunctions, Function, GcEffect, Instruction,
    IntrinsicTypeRepresentation, Layout, LayoutIdentity, LayoutKind, LirMeta, LirTargetProfile,
    LocalFunctionIdentities, LocalFunctionRef, ManagedCallDestination, ManagedPollSite,
    ManagedRuntimeFunction, MaterializationRoot, Module, NativeExternalMetadata,
    NativeGlobalBridges, OdrFreeLirFoundation, RefScan, RuntimeTypeMappingRecord,
    SafepointIdentities, SafepointIdentity, SafepointMappingRecord, SafepointSiteRef,
    SafepointSiteRole, ScoopAbiSignature, StatepointLiveSet, StructDefs, Terminator,
    TypeDescriptor, TypeDescriptorIdentity, TypeDescriptorRef, TypeDescriptorScan,
    VoidCallSignature, VtableRecord, WellKnownLayouts, WellKnownTypeDescriptors,
};

use super::Corruption;

pub(super) struct SemanticInputs {
    pub(super) module: Module,
    pub(super) foundation: OdrFreeLirFoundation,
    pub(super) definition: ObjectDefinitionPlanId,
    pub(super) safepoint_ids: Vec<u64>,
}

pub(super) fn inputs(corruption: Corruption) -> SemanticInputs {
    let (module, body, safepoints) = semantic_module();
    let foundation = foundation(&body, &safepoints, corruption);
    let definition = definition_plan(body.id());
    let safepoint_ids = safepoints
        .iter()
        .map(|identity| identity.runtime_id().get())
        .collect();
    SemanticInputs {
        module,
        foundation,
        definition,
        safepoint_ids,
    }
}

fn semantic_module() -> (Module, CallableBodyIdentity, Vec<SafepointIdentity>) {
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
        entry,
        meta: metadata(),
    };
    (module, body, safepoints)
}

fn foundation(
    body: &CallableBodyIdentity,
    safepoints: &[SafepointIdentity],
    corruption: Corruption,
) -> OdrFreeLirFoundation {
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
    canonical.set_definition_plans(vec![definition]).unwrap();
    canonical
        .set_definition_atoms(vec![primary, stackmap])
        .unwrap();
    canonical.set_symbol_requests(
        PersistentSymbolRequestTable::new(vec![body.symbol_request()]).unwrap(),
    );
    OdrFreeLirFoundation::try_new(ConeIdentity::SINGLE_FILE, canonical).unwrap()
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
    LirMeta {
        target_profile: LirTargetProfile::DARWIN_AARCH64,
        canonical_c_abi: CanonicalCAbiMetadata::default(),
        native_externals: NativeExternalMetadata::default(),
        well_known_layouts: WellKnownLayouts {
            string: string_layout,
        },
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
