use la_arena::Arena;
use scoop_codegen::StrongScoopLirObjectKindV1;
use scoop_identity::{
    CanonicalIdentifier, ConeIdentity, DeclarationScope, DefinitionOwnerChain, PackagePath,
    PersistentCallableBodyId, PersistentFunctionId, SourceDeclarationKey, SourceDeclarationSite,
};
use scoop_lir::*;

pub(super) fn module(
    cone: ConeIdentity,
    target_profile: LirTargetProfile,
    string: ExternalTypeDescriptor,
) -> Module {
    let mut external_type_descriptors = Arena::new();
    let string = TypeDescriptorRef::External(external_type_descriptors.alloc(string));
    Module {
        cone,
        globals: Arena::new(),
        initialization_units: Arena::new(),
        structs: StructDefs::default(),
        enums: EnumDefs::default(),
        functions: Vec::new(),
        extern_functions: ExternFunctions::default(),
        native_globals: Arena::new(),
        native_global_bridges: NativeGlobalBridges::default(),
        callback_bridges: Arena::new(),
        foreign_callback_families: Arena::new(),
        foreign_callback_bridges: Arena::new(),
        output: LirOutput::Library,
        meta: LirMeta {
            exact_types: Vec::new(),
            target_profile,
            canonical_c_abi: CanonicalCAbiMetadata::default(),
            native_externals: NativeExternalMetadata::default(),
            well_known_type_descriptors: WellKnownTypeDescriptors { string },
            arrays: Arena::new(),
            layouts: Arena::new(),
            type_descriptors: Arena::new(),
            external_type_descriptors,
            external_callables: Arena::new(),
        },
    }
}

pub(super) fn roundtrip(
    cone: ConeIdentity,
    descriptor: BoxedValueDescriptor,
    offsets: &[u64],
) -> Function {
    let zero_sized = matches!(descriptor, BoxedValueDescriptor::ZeroSized(_));
    let site = SourceDeclarationSite::new(
        cone,
        PackagePath::root(),
        DefinitionOwnerChain::top_level(),
        DeclarationScope::ConeWide,
    )
    .unwrap();
    let declaration = SourceDeclarationKey::function(
        site,
        CanonicalIdentifier::new(if zero_sized {
            "roundtripEmpty"
        } else {
            "roundtripValue"
        })
        .unwrap(),
        0,
        None,
        Vec::new(),
    );
    let callable_body = CallableBodyIdentity::for_function(
        PersistentFunctionId::from_source_declaration(&declaration).unwrap(),
    )
    .unwrap();
    let mut locals = Arena::new();
    let mut temps = Arena::new();
    let object = temps.alloc(Temp { ty: MANAGED_PTR });
    let mut instructions = Vec::new();
    let (payload, result, value, argument, returned, live) = match descriptor {
        BoxedValueDescriptor::ZeroSized(descriptor) => {
            let representation = descriptor.value().representation().clone();
            let out = temps.alloc(Temp {
                ty: representation.storage_type().clone(),
            });
            (
                BoxPayload::ZeroSized(descriptor.clone()),
                UnboxResult::ZeroSized { descriptor, out },
                Value::Temp(out),
                AbiArgument::ElidedZst(representation.clone()),
                AbiReturn::ElidedZst(representation),
                StatepointLiveSet::default(),
            )
        }
        BoxedValueDescriptor::NonZero(descriptor) => {
            let value = descriptor.value().clone();
            let source = locals.alloc(Local::new("source", LocalStorage::NonZero(value.clone())));
            let destination = locals.alloc(Local::new(
                "destination",
                LocalStorage::NonZero(value.clone()),
            ));
            instructions.push(Instruction::Store {
                local: source,
                value: Value::Param(0),
            });
            let live = if offsets.is_empty() {
                StatepointLiveSet::default()
            } else {
                StatepointLiveSet::new(vec![StatepointLiveValue {
                    source: CallerRootSource::Local(source),
                    ty: value.storage_type().clone(),
                    leaves: ManagedLeafPaths::new(
                        offsets
                            .iter()
                            .map(|offset| ManagedLeafPath {
                                byte_offset: *offset,
                            })
                            .collect(),
                    )
                    .unwrap(),
                }])
                .unwrap()
            };
            (
                BoxPayload::NonZero(descriptor.clone().bind_place(&locals, source).unwrap()),
                UnboxResult::NonZero(descriptor.bind_place(&locals, destination).unwrap()),
                Value::Local(destination),
                AbiArgument::Indirect(value.clone()),
                AbiReturn::Indirect(value),
                live,
            )
        }
    };
    let safepoint = SafepointSiteRef::from_u32(0);
    instructions.push(Instruction::BoxValue {
        out: object,
        payload,
        safepoint,
        live,
    });
    instructions.push(Instruction::UnboxValue {
        object: Value::Temp(object),
        result,
    });
    let mut blocks = Arena::new();
    let entry = blocks.alloc(BasicBlock {
        name: "entry".into(),
        instructions,
        terminator: Terminator::Return { value: Some(value) },
    });
    let safepoints = SafepointIdentities::checked(vec![(
        safepoint,
        SafepointIdentity::new(callable_body.id(), SafepointSiteRole::ManagedCall, 0).unwrap(),
    )])
    .unwrap();
    Function {
        callable_body,
        gc_effect: GcEffect::Managed,
        signature: ScoopAbiSignature::new(vec![argument], returned, CallingConvention::Cdecl),
        call_targets: CallTargets::default(),
        safepoints,
        locals,
        temps,
        blocks,
        entry,
    }
}

pub(super) fn check_ir(
    rendered: &[scoop_codegen::RenderedStrongObjectModuleV1],
    module: &Module,
    cases: &[(PersistentCallableBodyId, ExternalTypeDescriptorId, Vec<u64>)],
) {
    for (index, (body, descriptor, offsets)) in cases.iter().enumerate() {
        let member = rendered
            .iter()
            .find(|member| member.units().kind() == StrongScoopLirObjectKindV1::CallableBody(*body))
            .unwrap();
        let ir = member.llvm_ir();
        let symbol = module.meta.external_type_descriptors[*descriptor]
            .expected_symbol()
            .symbol();
        let declaration = ir
            .lines()
            .find(|line| {
                line.starts_with('@') && line.contains(symbol.as_str()) && line.contains(" = ")
            })
            .unwrap_or_else(|| panic!("missing external descriptor {symbol}: {ir}"));
        assert!(declaration.contains("external"), "{declaration}");
        if index == 0 {
            assert!(
                ir.contains("@scoop_rt_box_zst") && ir.contains("@scoop_rt_unbox_zst"),
                "{ir}"
            );
            assert!(
                !ir.contains("alloca") && !ir.contains("store {}") && !ir.contains("load {}"),
                "{ir}"
            );
            assert!(
                !ir.contains("@scoop_rt_box_value") && !ir.contains("native_region_roots"),
                "{ir}"
            );
        } else {
            assert!(
                ir.contains("@scoop_rt_box_value") && ir.contains("@scoop_rt_unbox_value"),
                "{ir}"
            );
            assert_eq!(
                ir.contains("native_region_roots"),
                !offsets.is_empty(),
                "{ir}"
            );
            if !offsets.is_empty() {
                assert!(
                    ir.contains("gc.statepoint") && ir.contains("gc.relocate"),
                    "{ir}"
                );
                assert!(
                    ir.contains("scoop_rt_push_native_region_roots")
                        && ir.contains("scoop_rt_pop_native_region_roots"),
                    "{ir}"
                );
            }
        }
        assert!(
            ir.lines()
                .any(|line| line.contains("scoop_rt_unbox_") && line.contains(symbol.as_str())),
            "{ir}"
        );
    }
}

pub(super) fn snapshot(module: &Module, references: bool) {
    let directory = crate::workspace_root().join("tests/fixtures/m23-external-boxing");
    let name = if references { "combined" } else { "standalone" };
    let path = directory.join(format!("{name}.lir.snap"));
    let actual = scoop_lir::dump(module);
    if std::env::var_os("SCOOP_UPDATE_EXTERNAL_BOXING").is_some() {
        std::fs::create_dir_all(&directory).unwrap();
        std::fs::write(&path, &actual).unwrap();
    }
    assert_eq!(actual, std::fs::read_to_string(path).unwrap());
}
