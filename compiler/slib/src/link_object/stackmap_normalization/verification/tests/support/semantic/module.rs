use super::*;

pub(super) fn semantic_module(
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
            | Corruption::StaticAliasedSentinels
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
        },
    });
    let module = Module {
        release_hooks: Default::default(),
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

pub(super) fn metadata() -> LirMeta {
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
        release_policy: Default::default(),
        relations: Default::default(),
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
