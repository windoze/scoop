use super::*;

pub(super) fn descriptor(
    meta: &mut LirMeta,
    name: &str,
    ty: LirType,
    storage: scoop_lir::ValueStorageLayoutV1,
) -> scoop_lir::BoxedValueDescriptor {
    let shape =
        TypeInstanceShapeV1::boxed_value(scoop_lir::LirTargetProfile::DARWIN_AARCH64, storage)
            .unwrap();
    let inline_scan = if shape.inline_scan().contains_reference() {
        scoop_lir::TypeDescriptorInlineScanV1::Defined(
            layout_identity(name, scoop_identity::RepresentationRole::ManagedValue)
                .scan_record()
                .id(),
        )
    } else {
        scoop_lir::TypeDescriptorInlineScanV1::Null
    };
    meta.layouts.alloc(Layout {
        identity: layout_identity(name, scoop_identity::RepresentationRole::ManagedValue),
        name: format!("{name} payload"),
        size: shape.inline_size(),
        align: shape.inline_alignment(),
        fields: vec![],
        c_layout: None,
        interior_mutable: false,
        kind: LayoutKind::Plain {
            scan: shape.inline_scan().clone(),
        },
    });
    let nominal = scoop_identity::PersistentTypeId::from_generated_key(
        &scoop_identity::GeneratedNominalKey::BoxedValue {
            payload: test_exact_type(name),
        },
    )
    .unwrap();
    let exact = scoop_identity::PersistentExactTypeId::from_key(
        &scoop_identity::ExactTypeKey::Nominal(nominal),
    )
    .unwrap();
    let root = scoop_lir::MaterializationRoot::cone_owned();
    let identity = scoop_lir::TypeDescriptorIdentity::new(
        scoop_lir::RuntimeTypeMappingRecord::new(exact).unwrap(),
        root.clone(),
    )
    .unwrap();
    let vtable = scoop_lir::VtableRecord::new(&identity, vec![]).unwrap();
    let id = meta.type_descriptors.alloc(TypeDescriptor {
        release_policy: Default::default(),
        relations: Default::default(),
        diagnostic_name: name.into(),
        identity,
        instance_layout: scoop_lir::LayoutIdentity::managed_object(
            exact,
            meta.target_profile,
            root,
        )
        .unwrap(),
        instance_shape: shape,
        inline_scan,
        parent: None,
        vtable,
        itables: vec![],
    });
    scoop_lir::BoxedValueDescriptor::from_local(
        &meta.type_descriptors,
        id,
        test_exact_type(name),
        ty,
    )
    .unwrap()
}

fn roundtrip_module(ty: LirType, size: u64, offsets: &[u64]) -> Module {
    let mut module = values_module();
    let scan = if offsets.is_empty() {
        RefScan::None
    } else {
        RefScan::References(offsets.to_vec())
    };
    let storage = if size == 0 {
        scoop_lir::ValueStorageLayoutV1::zero_sized(1)
    } else {
        scoop_lir::ValueStorageLayoutV1::inline(size, 8, scan)
    }
    .unwrap();
    let descriptor = descriptor(&mut module.meta, "BoxRoundtrip", ty.clone(), storage);
    let mut locals = Arena::new();
    let mut temps = Arena::new();
    let object = temps.alloc(Temp { ty: MANAGED_PTR });
    let mut instructions = Vec::new();
    let (payload, result, value, live) = match descriptor {
        scoop_lir::BoxedValueDescriptor::ZeroSized(descriptor) => {
            let out = temps.alloc(Temp { ty: ty.clone() });
            (
                scoop_lir::BoxPayload::ZeroSized(descriptor.clone()),
                scoop_lir::UnboxResult::ZeroSized { descriptor, out },
                Value::Temp(out),
                scoop_lir::StatepointLiveSet::default(),
            )
        }
        scoop_lir::BoxedValueDescriptor::NonZero(descriptor) => {
            let source = locals.alloc(Local::new(
                "source",
                scoop_lir::LocalStorage::NonZero(descriptor.value().clone()),
            ));
            let destination = locals.alloc(Local::new(
                "destination",
                scoop_lir::LocalStorage::NonZero(descriptor.value().clone()),
            ));
            instructions.push(Instruction::Store {
                local: source,
                value: Value::Param(0),
            });
            let live = if offsets.is_empty() {
                scoop_lir::StatepointLiveSet::default()
            } else {
                statepoint_live(vec![statepoint_value(
                    scoop_lir::CallerRootSource::Local(source),
                    ty.clone(),
                    offsets,
                )])
            };
            (
                scoop_lir::BoxPayload::NonZero(
                    descriptor.clone().bind_place(&locals, source).unwrap(),
                ),
                scoop_lir::UnboxResult::NonZero(
                    descriptor.bind_place(&locals, destination).unwrap(),
                ),
                Value::Local(destination),
                live,
            )
        }
    };
    instructions.push(Instruction::BoxValue {
        out: object,
        payload,
        safepoint: test_safepoint(1),
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
    module.functions = vec![Function {
        callable_body: callable_body("boxRoundtrip"),
        gc_effect: GcEffect::Managed,
        signature: plain_scoop_signature(vec![ty.clone()], ty),
        call_targets: CallTargets::default(),
        safepoints: scoop_lir::SafepointIdentities::default(),
        locals,
        temps,
        blocks,
        entry,
    }];
    module.output = scoop_lir::LirOutput::Library;
    refresh_module_safepoints(&mut module);
    module
}

fn codegen_error(module: &Module) -> String {
    let machine = host_target_machine().unwrap();
    let context = Context::create();
    emit_llvm_module(&context, module, &machine, host_profile())
        .unwrap_err()
        .0
}

#[test]
fn zero_sized_box_unbox_have_no_payload_storage_or_result_pointer() {
    let module = roundtrip_module(LirType::Aggregate(vec![]), 0, &[]);
    let ir = ir_of(&module);
    assert!(
        ir.contains("call ptr addrspace(1) @scoop_rt_box_zst(ptr @"),
        "{ir}"
    );
    assert!(
        ir.contains("call void @scoop_rt_unbox_zst(ptr addrspace(1) %box.t0, ptr @"),
        "{ir}"
    );
    assert!(
        !ir.contains("alloca") && !ir.contains("load {}") && !ir.contains("store {}"),
        "{ir}"
    );
    assert!(
        !ir.contains("scoop_rt_box_value") && !ir.contains("scoop_rt_push_native_region_roots"),
        "{ir}"
    );
    assert!(ir.contains("\"gc-leaf-function\""), "{ir}");
}

#[test]
fn reference_payload_roots_the_same_storage_before_the_managed_box() {
    let ty = LirType::Aggregate(vec![
        MANAGED_PTR,
        LirType::Aggregate(vec![MANAGED_PTR, LirType::I64]),
    ]);
    let module = roundtrip_module(ty, 24, &[0, 8]);
    let ir = ir_of(&module);
    let push = ir
        .find("call void @scoop_rt_push_native_region_roots")
        .unwrap();
    let boxed = ir
        .find("call ptr addrspace(1) @scoop_rt_box_value")
        .unwrap();
    let pop = ir
        .find("call void @scoop_rt_pop_native_region_roots")
        .unwrap();
    assert!(push < boxed && boxed < pop, "{ir}");
    assert!(ir[..push].contains("%box.inline_scan = load ptr"), "{ir}");
    assert!(ir[..push].contains("store ptr %box.inline_scan"), "{ir}");
    assert!(
        ir[..push].contains("store ptr %source, ptr %box.region.field"),
        "{ir}"
    );
    assert!(
        ir[boxed..]
            .lines()
            .next()
            .unwrap()
            .contains(", ptr %source)"),
        "{ir}"
    );
    assert!(
        ir.contains("call void @scoop_rt_unbox_value(ptr addrspace(1) %box.t0, ptr @"),
        "{ir}"
    );
    assert!(ir.contains(", ptr %destination)"), "{ir}");
    assert!(
        ir.contains("box.region.frame = alloca { ptr, ptr, i64 }"),
        "{ir}"
    );
    let rewritten = rewritten_ir_of(&module);
    assert!(
        rewritten.contains("gc.statepoint") && rewritten.contains("gc.relocate"),
        "{rewritten}"
    );
    assert!(
        rewritten.contains("call void @scoop_rt_push_native_region_roots"),
        "{rewritten}"
    );
    assert!(
        rewritten.contains("call void @scoop_rt_pop_native_region_roots"),
        "{rewritten}"
    );
}

#[test]
fn gc_free_payload_uses_the_descriptor_without_a_root_frame() {
    let module = roundtrip_module(LirType::I64, 8, &[]);
    let ir = ir_of(&module);
    assert!(
        ir.contains("@scoop_rt_box_value") && ir.contains("@scoop_rt_unbox_value"),
        "{ir}"
    );
    assert!(
        !ir.contains("native_region_roots") && !ir.contains("getelementptr i8, ptr %box.t0"),
        "{ir}"
    );
}

#[test]
fn box_descriptor_refinement_rejects_ordinary_class_and_wrong_storage() {
    let mut module = roundtrip_module(LirType::I64, 8, &[]);
    let descriptor = module
        .meta
        .type_descriptors
        .iter()
        .find(|(_, td)| td.diagnostic_name == "BoxRoundtrip")
        .unwrap()
        .0;
    module.meta.type_descriptors[descriptor].instance_shape =
        TypeInstanceShapeV1::fixed_object(module.meta.target_profile, 24, 8, RefScan::None)
            .unwrap();
    assert!(codegen_error(&module).contains("BoxedValue descriptor"));
    let mut module = roundtrip_module(LirType::I64, 8, &[]);
    let source = module.functions[0].locals.iter().next().unwrap().0;
    module.functions[0].locals[source] = test_local("source", LirType::I32);
    assert!(codegen_error(&module).contains("storage or BoxedValue descriptor"));
}

#[test]
fn box_call_cannot_omit_a_required_statepoint_root() {
    let mut module = roundtrip_module(MANAGED_PTR, 8, &[0]);
    for instruction in &mut module.functions[0]
        .blocks
        .iter_mut()
        .next()
        .unwrap()
        .1
        .instructions
    {
        if let Instruction::BoxValue { live, .. } = instruction {
            *live = Default::default();
        }
    }
    let error = codegen_error(&module);
    assert!(
        error.contains("box value") && error.contains("root"),
        "{error}"
    );
}

#[test]
fn ordinary_runtime_calls_cannot_bypass_the_box_payload_contract() {
    let mut module = values_module();
    let mut function = module.functions.remove(0);
    function.call_targets = CallTargets::default();
    let out = function.temps.alloc(Temp { ty: MANAGED_PTR });
    let site = direct_site(
        &mut function.call_targets,
        TestCallProtocol::Managed {
            safepoint: 1,
            destination: managed_runtime(scoop_lir::ManagedRuntimeFunction::BoxZst),
        },
        vec![METADATA_PTR],
        (MANAGED_PTR, RefScan::References(vec![0])),
        out,
        vec![Value::TypeDescriptor(
            module.meta.well_known_type_descriptors.string,
        )],
    );
    function.blocks = Arena::new();
    function.entry = function.blocks.alloc(BasicBlock {
        name: "entry".into(),
        instructions: vec![Instruction::Call { site }],
        terminator: Terminator::Return { value: None },
    });
    module.functions = vec![function];
    module.output = scoop_lir::LirOutput::Library;
    refresh_module_safepoints(&mut module);
    assert!(codegen_error(&module).contains("descriptor-refined operation"));
}

#[test]
fn descriptor_cannot_be_rebound_to_a_different_exact_payload_with_the_same_shape() {
    let module = roundtrip_module(LirType::I64, 8, &[]);
    let descriptor = module
        .meta
        .type_descriptors
        .iter()
        .find(|(_, descriptor)| descriptor.diagnostic_name == "BoxRoundtrip")
        .unwrap()
        .0;
    let result = scoop_lir::BoxedValueDescriptor::from_local(
        &module.meta.type_descriptors,
        descriptor,
        test_exact_type("AnotherLong"),
        LirType::I64,
    );
    assert_eq!(
        result,
        Err(scoop_lir::BoxDescriptorError::PayloadIdentityMismatch)
    );
}
