use la_arena::Arena;
use scoop_identity::{
    CanonicalIdentifier, CborIdentityRecord, ConeIdentity, DeclarationScope, DefinitionAtomRole,
    DefinitionAtomSubkey, DefinitionOwnerChain, DigestNodeId, DigestNodeKey, DigestPatchIntentKey,
    DigestSemanticFieldRole, ExactTypeKey, LinkageClass, ObjectDefinitionAtomId,
    ObjectDefinitionAtomKey, ObjectDefinitionPlanKey, PackagePath, PersistentExactTypeId,
    PersistentFunctionId, PersistentSymbolKey, PersistentSymbolRequest,
    PersistentSymbolRequestTable, PersistentTypeId, SourceDeclarationKey, SourceDeclarationSite,
    SourceNominalKind, StrongDefinitionEntity, StrongDefinitionRole,
};

use super::StrongSafepointRegistrationPlanSetV1;
use crate::{
    AbiReturn, BasicBlock, CallTarget, CallTargets, CallableBodyIdentity, CanonicalCAbiMetadata,
    CanonicalLirFoundation, DigestInputRefV1, DigestNodeV1, EnumDefs, ExternFunctions, Function,
    GcEffect, Instruction, IntrinsicTypeRepresentation, Layout, LayoutIdentity, LayoutKind,
    LirMeta, LirTargetProfile, LocalFunctionIdentities, LocalFunctionRef, ManagedCallDestination,
    ManagedLeafPath, ManagedLeafPaths, ManagedPollSite, ManagedRuntimeFunction,
    MaterializationRoot, Module, NativeExternalMetadata, NativeGlobalBridges, OdrFreeLirFoundation,
    RuntimeTypeMappingRecord, SafepointIdentities, SafepointIdentity, SafepointMappingRecord,
    SafepointSiteRef, SafepointSiteRole, ScoopAbiSignature, StatepointLiveSet, StatepointLiveValue,
    StrongDigestFinalizationPlanV1, StrongRegistrationIdentitySurfaceV1,
    StrongSafepointSemanticPlanSetV1, StructDefs, Terminator, TypeDescriptor,
    TypeDescriptorIdentity, TypeDescriptorRef, TypeInstanceShapeV1, VoidCallSignature,
    VtableRecord, WellKnownLayouts, WellKnownTypeDescriptors,
};

#[derive(Clone, Copy)]
pub(super) struct FixtureOptions {
    pub(super) semantic_producer: ConeIdentity,
    pub(super) registration_count: usize,
    pub(super) include_runtime_mappings: bool,
    pub(super) include_callable_owner: bool,
    pub(super) include_primary_atoms: bool,
    pub(super) include_symbols: bool,
    pub(super) include_object_nodes: bool,
    pub(super) include_stackmap_nodes: bool,
    pub(super) object_node_input: bool,
    pub(super) object_node_patch: bool,
    pub(super) exact_direct_inputs: bool,
    pub(super) extra_direct_input: bool,
    pub(super) registration_definition_patch: bool,
    pub(super) normalized_stackmap_patch: bool,
}

impl Default for FixtureOptions {
    fn default() -> Self {
        Self {
            semantic_producer: ConeIdentity::SINGLE_FILE,
            registration_count: 2,
            include_runtime_mappings: true,
            include_callable_owner: true,
            include_primary_atoms: true,
            include_symbols: true,
            include_object_nodes: true,
            include_stackmap_nodes: true,
            object_node_input: false,
            object_node_patch: false,
            exact_direct_inputs: true,
            extra_direct_input: false,
            registration_definition_patch: true,
            normalized_stackmap_patch: true,
        }
    }
}

pub(super) struct Fixture {
    pub(super) foundation: OdrFreeLirFoundation,
    pub(super) identities: StrongRegistrationIdentitySurfaceV1,
    pub(super) semantics: StrongSafepointSemanticPlanSetV1,
    pub(super) digests: StrongDigestFinalizationPlanV1,
    pub(super) primary_atoms: Vec<ObjectDefinitionAtomId>,
}

impl Fixture {
    pub(super) fn new(options: FixtureOptions) -> Self {
        assert!(options.registration_count <= 2);
        let (module, body, safepoints) = semantic_module(options.semantic_producer);
        let semantics = StrongSafepointSemanticPlanSetV1::from_module(&module).unwrap();
        let registrations = safepoints
            .iter()
            .take(options.registration_count)
            .map(|identity| registration_artifacts(identity.site_id()))
            .collect::<Vec<_>>();

        let mut canonical = CanonicalLirFoundation::empty();
        if options.include_callable_owner {
            canonical
                .set_callable_bodies(vec![body.identity_record().clone()])
                .unwrap();
        }
        canonical
            .set_safepoint_sites(
                safepoints
                    .iter()
                    .map(|identity| identity.site_record().clone())
                    .collect(),
            )
            .unwrap();
        if options.include_runtime_mappings {
            canonical
                .set_safepoints(
                    safepoints
                        .iter()
                        .map(SafepointMappingRecord::from_identity)
                        .collect(),
                )
                .unwrap();
        }
        canonical
            .set_definition_plans(
                registrations
                    .iter()
                    .map(|registration| registration.plan.clone())
                    .collect(),
            )
            .unwrap();
        if options.include_primary_atoms {
            canonical
                .set_definition_atoms(
                    registrations
                        .iter()
                        .map(|registration| registration.primary.clone())
                        .collect(),
                )
                .unwrap();
        }
        if options.include_symbols {
            canonical.set_symbol_requests(
                PersistentSymbolRequestTable::new(
                    registrations
                        .iter()
                        .map(|registration| registration.symbol)
                        .collect(),
                )
                .unwrap(),
            );
        }
        let foundation =
            OdrFreeLirFoundation::try_new(ConeIdentity::SINGLE_FILE, canonical).unwrap();

        let extra_source = (options.extra_direct_input || options.object_node_input).then(|| {
            DigestNodeV1::new(
                DigestNodeKey::source_signature(body.id()),
                Vec::new(),
                Vec::new(),
            )
            .unwrap()
        });
        let mut nodes = Vec::new();
        let mut image_inputs = Vec::new();
        for registration in &registrations {
            let object = options.include_object_nodes.then(|| {
                let direct_inputs = options
                    .object_node_input
                    .then(|| DigestInputRefV1::from_node(extra_source.as_ref().unwrap()))
                    .into_iter()
                    .collect();
                let key = DigestNodeKey::object_definition(registration.primary.id());
                let source = DigestNodeId::from_key(&key).unwrap();
                let patches = options
                    .object_node_patch
                    .then(|| {
                        DigestPatchIntentKey::new(
                            source,
                            registration.plan.id(),
                            DefinitionAtomRole::Primary,
                            DigestSemanticFieldRole::DescriptorDefinition,
                        )
                    })
                    .into_iter()
                    .collect();
                DigestNodeV1::new(key, direct_inputs, patches).unwrap()
            });
            let stackmap = options.include_stackmap_nodes.then(|| {
                let key = DigestNodeKey::stackmap_record(registration.site);
                let source = DigestNodeId::from_key(&key).unwrap();
                let patches = options
                    .normalized_stackmap_patch
                    .then(|| {
                        DigestPatchIntentKey::new(
                            source,
                            registration.plan.id(),
                            DefinitionAtomRole::Primary,
                            DigestSemanticFieldRole::NormalizedStackmap,
                        )
                    })
                    .into_iter()
                    .collect();
                DigestNodeV1::new(key, Vec::new(), patches).unwrap()
            });
            let key = DigestNodeKey::strong_registration(registration.plan.id());
            let source = DigestNodeId::from_key(&key).unwrap();
            let mut direct_inputs: Vec<DigestInputRefV1> = if options.exact_direct_inputs {
                object
                    .iter()
                    .chain(stackmap.iter())
                    .map(DigestInputRefV1::from_node)
                    .collect()
            } else {
                stackmap.iter().map(DigestInputRefV1::from_node).collect()
            };
            direct_inputs.extend(extra_source.iter().map(DigestInputRefV1::from_node));
            let patches = options
                .registration_definition_patch
                .then(|| {
                    DigestPatchIntentKey::new(
                        source,
                        registration.plan.id(),
                        DefinitionAtomRole::Primary,
                        DigestSemanticFieldRole::RegistrationDefinition,
                    )
                })
                .into_iter()
                .collect();
            let fingerprint = DigestNodeV1::new(key, direct_inputs, patches).unwrap();
            image_inputs.push(DigestInputRefV1::from_node(&fingerprint));
            nodes.extend(object);
            nodes.extend(stackmap);
            nodes.push(fingerprint);
        }
        nodes.extend(extra_source);
        nodes.push(
            DigestNodeV1::new(
                DigestNodeKey::runtime_image(ConeIdentity::SINGLE_FILE),
                image_inputs,
                Vec::new(),
            )
            .unwrap(),
        );
        let digests = StrongDigestFinalizationPlanV1::new(nodes, &foundation).unwrap();
        let identities =
            StrongRegistrationIdentitySurfaceV1::from_foundation(&foundation, &digests).unwrap();
        let primary_atoms = registrations
            .iter()
            .map(|registration| registration.primary.id())
            .collect();

        Self {
            foundation,
            identities,
            semantics,
            digests,
            primary_atoms,
        }
    }

    pub(super) fn build(
        &self,
    ) -> Result<
        StrongSafepointRegistrationPlanSetV1,
        super::StrongSafepointRegistrationPlanBuildError,
    > {
        StrongSafepointRegistrationPlanSetV1::new(
            &self.foundation,
            &self.identities,
            &self.semantics,
            &self.digests,
        )
    }
}

struct RegistrationArtifacts {
    site: scoop_identity::PersistentSafepointSiteId,
    plan: CborIdentityRecord<
        scoop_identity::ObjectDefinitionPlanId,
        scoop_identity::ObjectDefinitionPlanKey,
    >,
    primary: CborIdentityRecord<ObjectDefinitionAtomId, ObjectDefinitionAtomKey>,
    symbol: PersistentSymbolRequest,
}

fn registration_artifacts(
    site: scoop_identity::PersistentSafepointSiteId,
) -> RegistrationArtifacts {
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
        name: "entry".to_string(),
        instructions: vec![
            poll(
                target,
                SafepointSiteRef::from_u32(0),
                StatepointLiveSet::default(),
            ),
            poll(target, SafepointSiteRef::from_u32(1), live_roots()),
        ],
        terminator: Terminator::Return { value: None },
    });
    let function = Function {
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
            target,
            safepoint,
            live,
        },
    }
}

fn live_roots() -> StatepointLiveSet {
    StatepointLiveSet::new(vec![StatepointLiveValue {
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
        parent: None,
        vtable,
        itables: Vec::new(),
    });
    LirMeta {
        exact_types: Vec::new(),
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
