use la_arena::Arena;
use scoop_identity::{
    CanonicalIdentifier, ConeIdentity, DeclarationScope, DefinitionOwnerChain, DigestNodeKey,
    ExactTypeKey, PackagePath, PersistentExactTypeId, PersistentFunctionId, PersistentTypeId,
    SourceDeclarationKey, SourceDeclarationSite, SourceNominalKind,
};

use super::StrongSafepointProductionPlanSetV1;
use crate::{
    AbiReturn, BasicBlock, CallTarget, CallTargets, CallableBodyIdentity, CanonicalCAbiMetadata,
    CanonicalLirFoundation, ConeLirFoundation, DigestFinalizationPlanV1, DigestInputRefV1,
    DigestNodeV1, EnumDefs, ExternFunctions, Function, GcEffect, Instruction,
    IntrinsicTypeRepresentation, Layout, LayoutIdentity, LayoutKind, LirMeta,
    LocalFunctionIdentities, LocalFunctionRef, ManagedCallDestination, ManagedLeafPath,
    ManagedLeafPaths, ManagedPollSite, ManagedRuntimeFunction, MaterializationRoot, Module,
    NativeExternalMetadata, NativeGlobalBridges, RuntimeTypeMappingRecord, SafepointIdentities,
    SafepointIdentity, SafepointMappingRecord, SafepointSiteRef, SafepointSiteRole,
    ScoopAbiSignature, StatepointLiveSet, StatepointLiveValue, StrongSafepointSemanticPlanSetV1,
    StructDefs, Terminator, TypeDescriptor, TypeDescriptorIdentity, TypeDescriptorRef,
    TypeInstanceShapeV1, VoidCallSignature, VtableRecord, WellKnownTypeDescriptors,
};

pub(super) struct Fixture {
    pub(super) foundation: ConeLirFoundation,
    pub(super) semantics: StrongSafepointSemanticPlanSetV1,
    pub(super) digests: DigestFinalizationPlanV1,
}

impl Fixture {
    pub(super) fn new() -> Self {
        let (module, body, sites) = semantic_module(ConeIdentity::SINGLE_FILE);
        let semantics = StrongSafepointSemanticPlanSetV1::from_module(&module).unwrap();
        let mut canonical = CanonicalLirFoundation::empty();
        canonical
            .set_callable_bodies(vec![body.identity_record().clone()])
            .unwrap();
        canonical
            .set_safepoint_sites(
                sites
                    .iter()
                    .map(|site| site.site_record().clone())
                    .collect(),
            )
            .unwrap();
        canonical
            .set_safepoints(
                sites
                    .iter()
                    .map(SafepointMappingRecord::from_identity)
                    .collect(),
            )
            .unwrap();
        let foundation = ConeLirFoundation::try_new(ConeIdentity::SINGLE_FILE, canonical).unwrap();
        let mut nodes = sites
            .iter()
            .map(|site| {
                DigestNodeV1::new(DigestNodeKey::stackmap_record(site.site_id()), Vec::new())
                    .unwrap()
            })
            .collect::<Vec<_>>();
        nodes.push(
            DigestNodeV1::new(
                DigestNodeKey::runtime_image(ConeIdentity::SINGLE_FILE),
                nodes.iter().map(DigestInputRefV1::from_node).collect(),
            )
            .unwrap(),
        );
        let digests = DigestFinalizationPlanV1::new(nodes, &foundation).unwrap();
        Self {
            foundation,
            semantics,
            digests,
        }
    }

    pub(super) fn build(
        &self,
    ) -> Result<StrongSafepointProductionPlanSetV1, super::StrongSafepointProductionPlanBuildError>
    {
        StrongSafepointProductionPlanSetV1::new(&self.semantics, &self.digests)
    }
}

fn semantic_module(
    producer: ConeIdentity,
) -> (Module, CallableBodyIdentity, Vec<SafepointIdentity>) {
    let body = CallableBodyIdentity::for_function(function_id("registrationOwner")).unwrap();
    let safepoints = vec![
        SafepointIdentity::new(body.id(), SafepointSiteRole::ManagedPoll, 0).unwrap(),
        SafepointIdentity::new(body.id(), SafepointSiteRole::ManagedPoll, 1).unwrap(),
    ];
    let mut call_targets = CallTargets::default();
    let signature = call_targets.void_signatures.alloc(VoidCallSignature::new(
        Vec::new(),
        crate::CallingConvention::Cdecl,
    ));
    let target = call_targets.managed_targets.void.alloc(CallTarget {
        destination: ManagedCallDestination::runtime(ManagedRuntimeFunction::Safepoint),
        signature,
    });
    let mut blocks = Arena::new();
    let entry = blocks.alloc(BasicBlock {
        terminator_origin: Default::default(),
        name: "entry".to_string(),
        instructions: vec![
            poll(
                target,
                SafepointSiteRef::from_u32(0),
                StatepointLiveSet::default(),
            ),
            poll(target, SafepointSiteRef::from_u32(1), live_roots()),
        ]
        .into(),
        terminator: Terminator::Return { value: None },
    });
    let function = Function {
        debug_name: Default::default(),
        debug_variables: Vec::new(),
        debug_signature: Default::default(),
        debug_origin: Default::default(),
        callable_body: body.clone(),
        gc_effect: GcEffect::Managed,
        signature: ScoopAbiSignature::new(
            Vec::new(),
            AbiReturn::UnitVoid,
            crate::CallingConvention::Cdecl,
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
        release_hooks: Arena::new(),
        cone: producer,
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
        output: crate::LirOutput::Executable { entry },
        meta: metadata(),
    };
    (module, body, safepoints)
}

fn poll(
    target: crate::VoidCallTargetId<ManagedCallDestination>,
    safepoint: SafepointSiteRef,
    live: StatepointLiveSet,
) -> Instruction {
    Instruction::ManagedPoll {
        site: ManagedPollSite {
            kind: crate::ManagedPollKind::Full,
            target,
            safepoint,
            live,
        },
    }
}

fn live_roots() -> StatepointLiveSet {
    StatepointLiveSet::new(vec![StatepointLiveValue {
        restore: true,
        source: crate::CallerRootSource::Param(0),
        ty: crate::LirType::Aggregate(vec![crate::MANAGED_PTR; 2]),
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
            crate::target_fixtures::DARWIN_AARCH64,
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
        release_policy: Default::default(),
        relations: Default::default(),
        diagnostic_name: "String".to_string(),
        identity,
        instance_layout: layouts[string_layout].identity.clone(),
        instance_shape: TypeInstanceShapeV1::inline_bytes(crate::target_fixtures::DARWIN_AARCH64)
            .unwrap(),
        inline_scan: crate::TypeDescriptorInlineScanV1::Null,
        parent: None,
        vtable,
        itables: Vec::new(),
    });
    LirMeta {
        profile_bodies: Vec::new(),
        debug_types: Default::default(),
        debug_globals: Default::default(),
        exact_types: Vec::new(),
        target_profile: crate::target_fixtures::DARWIN_AARCH64,
        canonical_c_abi: CanonicalCAbiMetadata::default(),
        native_externals: NativeExternalMetadata::default(),
        well_known_type_descriptors: WellKnownTypeDescriptors {
            string: TypeDescriptorRef::Local(string_descriptor),
        },
        arrays: Arena::new(),
        layouts,
        type_descriptors,
        external_type_descriptors: Arena::new(),
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

fn source_site() -> SourceDeclarationSite {
    SourceDeclarationSite::new(
        ConeIdentity::SINGLE_FILE,
        PackagePath::root(),
        DefinitionOwnerChain::top_level(),
        DeclarationScope::ConeWide,
    )
    .unwrap()
}
