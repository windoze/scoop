use super::*;

/// An M6-shaped module: class TypeDescriptors (parent chain, ordinary
/// user-method vtables with no compiler-owned `Any` slots, one itable) and
/// indirect calls through a table pointer (vtable / itable dispatch shape,
/// impl spec 2.9).
pub(super) fn classes_module() -> Module {
    classes_module_for(scoop_lir::LirTargetProfile::DARWIN_AARCH64)
}

pub(super) fn classes_module_for(target: scoop_lir::LirTargetProfile) -> Module {
    // `fn describe(this: ptr) -> ptr` shared shape: returns `this`.
    let describe = |symbol: &str| {
        let mut blocks = Arena::default();
        let entry = blocks.alloc(BasicBlock {
            name: "entry".to_string(),
            instructions: vec![],
            terminator: Terminator::Return {
                value: Some(Value::Param(0)),
            },
        });
        Function {
            callable_body: callable_body(symbol),
            safepoints: scoop_lir::SafepointIdentities::default(),
            gc_effect: GcEffect::Managed,
            signature: plain_scoop_signature(vec![MANAGED_PTR], MANAGED_PTR),
            call_targets: CallTargets::default(),
            locals: Arena::default(),
            temps: Arena::default(),
            blocks,
            entry,
        }
    };

    // fun @scoop_main(table: ptr, obj: ptr) -> ptr:
    //   t0 = call_indirect table[0](obj) : ptr   (result)
    //   call_indirect table[1](obj)              (void)
    //   ret t0
    let mut temps = Arena::default();
    let t0 = temps.alloc(Temp { ty: MANAGED_PTR });
    let mut call_targets = CallTargets::default();
    let result_dispatch = dispatch_destination(&mut call_targets, Value::Param(0), 0);
    let mut result_call = direct_site(
        &mut call_targets,
        TestCallProtocol::Managed {
            safepoint: 1,
            destination: result_dispatch,
        },
        vec![MANAGED_PTR],
        (MANAGED_PTR, RefScan::References(vec![0])),
        t0,
        vec![Value::Param(1)],
    );
    set_managed_live(
        &mut result_call,
        statepoint_live(vec![statepoint_value(
            scoop_lir::CallerRootSource::Param(1),
            MANAGED_PTR,
            &[0],
        )]),
    );
    let void_dispatch = dispatch_destination(&mut call_targets, Value::Param(0), 1);
    let mut void_call = void_site(
        &mut call_targets,
        TestCallProtocol::Managed {
            safepoint: 2,
            destination: void_dispatch,
        },
        vec![MANAGED_PTR],
        vec![Value::Param(1)],
    );
    set_managed_live(
        &mut void_call,
        statepoint_live(vec![
            statepoint_value(scoop_lir::CallerRootSource::Param(1), MANAGED_PTR, &[0]),
            statepoint_value(scoop_lir::CallerRootSource::Temp(t0), MANAGED_PTR, &[0]),
        ]),
    );
    let mut blocks = Arena::default();
    let entry = blocks.alloc(BasicBlock {
        name: "entry".to_string(),
        instructions: vec![
            Instruction::Call { site: result_call },
            Instruction::Call { site: void_call },
        ],
        terminator: Terminator::Return {
            value: Some(Value::Temp(t0)),
        },
    });
    let main = Function {
        callable_body: callable_body_at(file!(), line!()),
        safepoints: scoop_lir::SafepointIdentities::default(),
        gc_effect: GcEffect::Managed,
        signature: plain_scoop_signature(vec![METADATA_PTR, MANAGED_PTR], MANAGED_PTR),
        call_targets,
        locals: Arena::default(),
        temps,
        blocks,
        entry,
    };

    let mut meta = string_metadata_for(target);
    let describable = meta.type_descriptors.alloc(TypeDescriptor {
        release_policy: Default::default(),
        relations: Default::default(),
        diagnostic_name: "Describable".to_string(),
        identity: type_descriptor_identity("Describable"),
        instance_layout: scoop_lir::LayoutIdentity::managed_object(
            test_exact_type("Describable"),
            target,
            scoop_lir::MaterializationRoot::cone_owned(),
        )
        .unwrap(),
        instance_shape: TypeInstanceShapeV1::abstract_ref(),
        inline_scan: scoop_lir::TypeDescriptorInlineScanV1::Null,
        parent: None,
        vtable: vtable("Describable", vec![]),
        itables: vec![],
    });
    let shape = meta.type_descriptors.alloc(TypeDescriptor {
        release_policy: Default::default(),
        relations: Default::default(),
        diagnostic_name: "Shape".to_string(),
        identity: type_descriptor_identity("Shape"),
        instance_layout: scoop_lir::LayoutIdentity::managed_object(
            test_exact_type("Shape"),
            target,
            scoop_lir::MaterializationRoot::cone_owned(),
        )
        .unwrap(),
        instance_shape: TypeInstanceShapeV1::fixed_object(
            target,
            24,
            8,
            RefScan::References(vec![16]),
        )
        .unwrap(),
        inline_scan: scoop_lir::TypeDescriptorInlineScanV1::Null,
        parent: None,
        vtable: vtable(
            "Shape",
            vec![DispatchEntry {
                callable: CallableRef::Local(scoop_lir::LocalFunctionId::from_u32(0)),
            }],
        ),
        itables: vec![],
    });
    meta.type_descriptors.alloc(TypeDescriptor {
        release_policy: Default::default(),
        relations: Default::default(),
        diagnostic_name: "Point".to_string(),
        identity: type_descriptor_identity("Point"),
        instance_layout: scoop_lir::LayoutIdentity::managed_object(
            test_exact_type("Point"),
            target,
            scoop_lir::MaterializationRoot::cone_owned(),
        )
        .unwrap(),
        instance_shape: TypeInstanceShapeV1::fixed_object(
            target,
            32,
            8,
            RefScan::References(vec![16]),
        )
        .unwrap(),
        inline_scan: scoop_lir::TypeDescriptorInlineScanV1::Null,
        parent: Some(TypeDescriptorRef::Local(shape)),
        vtable: vtable(
            "Point",
            vec![DispatchEntry {
                callable: CallableRef::Local(scoop_lir::LocalFunctionId::from_u32(1)),
            }],
        ),
        itables: vec![itable(
            "Point",
            "Describable",
            TypeDescriptorRef::Local(describable),
            vec![DispatchEntry {
                callable: CallableRef::Local(scoop_lir::LocalFunctionId::from_u32(1)),
            }],
        )],
    });

    let mut module = Module {
        release_hooks: Default::default(),
        cone: scoop_identity::ConeIdentity::SINGLE_FILE,
        globals: Arena::default(),
        initialization_units: Arena::default(),
        structs: scoop_lir::StructDefs::default(),
        enums: scoop_lir::EnumDefs::default(),
        extern_functions: Default::default(),
        native_globals: Arena::default(),
        native_global_bridges: Default::default(),
        callback_bridges: Arena::default(),
        foreign_callback_families: Arena::default(),
        foreign_callback_bridges: Arena::default(),
        functions: vec![describe("Shape.describe"), describe("Point.describe"), main],
        output: scoop_lir::LirOutput::Executable {
            entry: managed_function_ref(2),
        },
        meta,
    };
    refresh_module_safepoints(&mut module);
    append_executable_entry(&mut module, "classes-executable-entry");
    module
}

#[test]
fn emits_m6_type_descriptors_and_call_indirect() {
    let module = classes_module();
    let output =
        std::env::temp_dir().join(format!("scoop_codegen_m6_test_{}.o", std::process::id()));
    write_verified_test_object(&module, &output);
    let len = std::fs::metadata(&output)
        .expect("object file exists")
        .len();
    assert!(len > 0, "object file is empty");
    std::fs::remove_file(&output).ok();
}

/// An M6 heap-access module with a typed TypeDescriptor operand,
/// HeapStore field writes, HeapLoad reads (header, i64
/// field, ptr field, TD vtable pointer), and a descriptor-refined box operation
/// with a typed aggregate payload local.
pub(super) fn heap_module() -> Module {
    heap_module_for(scoop_lir::LirTargetProfile::DARWIN_AARCH64)
}

pub(super) fn heap_module_for(target: scoop_lir::LirTargetProfile) -> Module {
    let globals = Arena::default();
    let mut meta = string_metadata_for(target);
    let point_descriptor = meta.type_descriptors.alloc(TypeDescriptor {
        release_policy: Default::default(),
        relations: Default::default(),
        diagnostic_name: "Point".to_string(),
        identity: type_descriptor_identity("Point"),
        instance_layout: scoop_lir::LayoutIdentity::managed_object(
            test_exact_type("Point"),
            target,
            scoop_lir::MaterializationRoot::cone_owned(),
        )
        .unwrap(),
        instance_shape: TypeInstanceShapeV1::fixed_object(
            target,
            32,
            8,
            RefScan::References(vec![24]),
        )
        .unwrap(),
        inline_scan: scoop_lir::TypeDescriptorInlineScanV1::Null,
        parent: None,
        vtable: vtable(
            "Point",
            vec![DispatchEntry {
                callable: CallableRef::Local(scoop_lir::LocalFunctionId::from_u32(0)),
            }],
        ),
        itables: vec![],
    });
    let point_descriptor = TypeDescriptorRef::Local(point_descriptor);

    // fun @Point.describe(this: ptr) -> ptr: returns `this` (vtable
    // slot material).
    let mut describe_blocks = Arena::default();
    let describe_entry = describe_blocks.alloc(BasicBlock {
        name: "entry".to_string(),
        instructions: vec![],
        terminator: Terminator::Return {
            value: Some(Value::Param(0)),
        },
    });
    let describe = Function {
        callable_body: callable_body_at(file!(), line!()),
        safepoints: scoop_lir::SafepointIdentities::default(),
        gc_effect: GcEffect::Managed,
        signature: plain_scoop_signature(vec![MANAGED_PTR], MANAGED_PTR),
        call_targets: CallTargets::default(),
        locals: Arena::default(),
        temps: Arena::default(),
        blocks: describe_blocks,
        entry: describe_entry,
    };

    // fun @scoop_main() -> void (M9 16-byte header, fields at byte
    // offsets 16 and 24):
    //   t0 = scoop_rt_alloc(@TypeDescriptor(Point), 32)  (typed TD operand)
    //   heap_store t0 +16, 42     (i64 field)
    //   heap_store t0 +24, t0     (ptr field)
    //   t1 = heap_load t0 +0 : ptr   (object header: the TD)
    //   t2 = heap_load t0 +16 : i64  (field 1)
    //   t3 = heap_load t0 +24 : ptr  (field 2)
    //   t4 = heap_load t1 +40 : ptr  (TD field 5: the vtable pointer)
    //   t5 = aggregate (t2) : {i64}
    //   t6 = box_value(@TypeDescriptor(BoxedPayload), payload)
    //   t7 = scoop_rt_is_instance(t6, @TypeDescriptor(Point)) : i1
    //   call_indirect t4[0](t3); println_int t2; println_boolean t7
    let mut temps = Arena::default();
    let t0 = temps.alloc(Temp { ty: MANAGED_PTR });
    let t1 = temps.alloc(Temp { ty: METADATA_PTR });
    let t2 = temps.alloc(Temp { ty: LirType::I64 });
    let t3 = temps.alloc(Temp { ty: MANAGED_PTR });
    let t4 = temps.alloc(Temp { ty: METADATA_PTR });
    let t5 = temps.alloc(Temp {
        ty: LirType::Aggregate(vec![LirType::I64]),
    });
    let t6 = temps.alloc(Temp { ty: MANAGED_PTR });
    let t7 = temps.alloc(Temp { ty: LirType::I1 });
    let mut locals = Arena::default();
    let payload = locals.alloc(test_local(
        "box_payload",
        LirType::Aggregate(vec![LirType::I64]),
    ));
    let byte_size_ty = LirType::MachineScalar(MachineScalarKind::ByteSize);
    let mut call_targets = CallTargets::default();
    let alloc_site = direct_site(
        &mut call_targets,
        TestCallProtocol::Managed {
            safepoint: 1,
            destination: managed_runtime(scoop_lir::ManagedRuntimeFunction::Alloc),
        },
        vec![METADATA_PTR, byte_size_ty.clone()],
        (MANAGED_PTR, RefScan::References(vec![0])),
        t0,
        vec![
            Value::TypeDescriptor(point_descriptor),
            Value::MachineScalar(MachineScalarValue::ByteSize(32)),
        ],
    );
    let boxed = super::boxing::descriptor(
        &mut meta,
        "BoxedPayload",
        LirType::Aggregate(vec![LirType::I64]),
        scoop_lir::ValueStorageLayoutV1::inline(8, 8, RefScan::None).unwrap(),
    );
    let scoop_lir::BoxedValueDescriptor::NonZero(boxed) = boxed else {
        unreachable!()
    };
    let box_instruction = Instruction::BoxValue {
        out: t6,
        payload: scoop_lir::BoxPayload::NonZero(boxed.bind_place(&locals, payload).unwrap()),
        safepoint: test_safepoint(2),
        live: statepoint_live(vec![statepoint_value(
            scoop_lir::CallerRootSource::Temp(t3),
            MANAGED_PTR,
            &[0],
        )]),
    };
    let is_instance_site = direct_site(
        &mut call_targets,
        TestCallProtocol::NoGc {
            destination: no_gc_runtime(scoop_lir::NoGcRuntimeFunction::IsInstance),
        },
        vec![MANAGED_PTR, METADATA_PTR],
        (LirType::I1, RefScan::None),
        t7,
        vec![Value::Temp(t6), Value::TypeDescriptor(point_descriptor)],
    );
    let dispatch = dispatch_destination(&mut call_targets, Value::Temp(t4), 0);
    let mut dispatch_site = void_site(
        &mut call_targets,
        TestCallProtocol::Managed {
            safepoint: 3,
            destination: dispatch,
        },
        vec![MANAGED_PTR],
        vec![Value::Temp(t3)],
    );
    set_managed_live(
        &mut dispatch_site,
        statepoint_live(vec![statepoint_value(
            scoop_lir::CallerRootSource::Temp(t3),
            MANAGED_PTR,
            &[0],
        )]),
    );
    let mut blocks = Arena::default();
    let entry = blocks.alloc(BasicBlock {
        name: "entry".to_string(),
        instructions: vec![
            Instruction::Call { site: alloc_site },
            Instruction::HeapStore {
                object: Value::Temp(t0),
                offset: 16,
                value: signed64(42),
            },
            Instruction::HeapStore {
                object: Value::Temp(t0),
                offset: 24,
                value: Value::Temp(t0),
            },
            Instruction::HeapLoad {
                out: t1,
                object: Value::Temp(t0),
                offset: 0,
            },
            Instruction::HeapLoad {
                out: t2,
                object: Value::Temp(t0),
                offset: 16,
            },
            Instruction::HeapLoad {
                out: t3,
                object: Value::Temp(t0),
                offset: 24,
            },
            Instruction::HeapLoad {
                out: t4,
                object: Value::Temp(t1),
                offset: 40,
            },
            Instruction::MakeAggregate {
                out: t5,
                elements: vec![Value::Temp(t2)],
            },
            Instruction::Store {
                local: payload,
                value: Value::Temp(t5),
            },
            box_instruction,
            Instruction::Call {
                site: is_instance_site,
            },
            Instruction::Call {
                site: dispatch_site,
            },
        ],
        terminator: Terminator::Return { value: None },
    });
    let main = Function {
        callable_body: callable_body_at(file!(), line!()),
        safepoints: scoop_lir::SafepointIdentities::default(),
        gc_effect: GcEffect::Managed,
        signature: plain_scoop_signature(vec![], LirType::Void),
        call_targets,
        locals,
        temps,
        blocks,
        entry,
    };

    let mut module = Module {
        release_hooks: Default::default(),
        cone: scoop_identity::ConeIdentity::SINGLE_FILE,
        globals,
        initialization_units: Arena::default(),
        structs: scoop_lir::StructDefs::default(),
        enums: scoop_lir::EnumDefs::default(),
        extern_functions: Default::default(),
        native_globals: Arena::default(),
        native_global_bridges: Default::default(),
        callback_bridges: Arena::default(),
        foreign_callback_families: Arena::default(),
        foreign_callback_bridges: Arena::default(),
        functions: vec![describe, main],
        output: scoop_lir::LirOutput::Executable {
            entry: managed_function_ref(1),
        },
        meta,
    };
    refresh_module_safepoints(&mut module);
    module
}

fn keep_heap_object_live_for_appended_access(function: &mut Function, object: TempId) {
    let mut object_defined = false;
    for instruction in &mut function.blocks[function.entry].instructions {
        if let Instruction::Call { site } = instruction
            && site.result() == scoop_lir::TypedCallResult::Direct(object)
        {
            object_defined = true;
            continue;
        }
        if !object_defined {
            continue;
        }
        let roots = match instruction {
            Instruction::Call {
                site: CallSite::Managed(site),
            } => &mut site.live,
            Instruction::BoxValue { live, .. } => live,
            _ => continue,
        };

        let mut live = vec![statepoint_value(
            scoop_lir::CallerRootSource::Temp(object),
            MANAGED_PTR,
            &[0],
        )];
        live.extend_from_slice(roots.as_slice());
        *roots = statepoint_live(live);
    }
    assert!(
        object_defined,
        "heap fixture must define its object by a call"
    );
}

#[test]
fn emits_m6_heap_access_and_typed_descriptors() {
    let module = heap_module();
    let ir = ir_of(&module);
    assert!(
        ir.contains("@scoop_rt_allocation_context = external thread_local global ptr")
            && ir.contains("alloc.fast.0")
            && ir.contains("alloc.slow.0")
            && ir.contains("@scoop_runtime_finish_tlab_alloc")
            && ir.contains("@scoop_runtime_alloc_slow"),
        "managed allocation must expose an inline TLAB fast path and collecting fallback:\n{ir}"
    );
    assert!(
        !ir.contains("call ptr @scoop_rt_alloc"),
        "generated code must not route every allocation through the compatibility entry:\n{ir}"
    );
    assert!(
        ir.contains("%alloc_is_regular = icmp ule i64 %alloc_size, 32640")
            && !ir.contains("tlab_line_base"),
        "the inline allocator must accept contiguous objects across Immix lines:\n{ir}"
    );
    let output = std::env::temp_dir().join(format!(
        "scoop_codegen_m6_heap_test_{}.o",
        std::process::id()
    ));
    write_verified_test_object(&module, &output);
    let len = std::fs::metadata(&output)
        .expect("object file exists")
        .len();
    assert!(len > 0, "object file is empty");
    std::fs::remove_file(&output).ok();
}

#[test]
fn emits_typed_machine_heap_state_access() {
    let mut module = heap_module();
    let function = &mut module.functions[1];
    let object = function.temps.iter().next().expect("allocated object").0;
    let state = function.temps.alloc(Temp {
        ty: LirType::MachineScalar(MachineScalarKind::CoroutineFrameState),
    });
    function.blocks[function.entry].instructions.extend([
        Instruction::MachineHeapStore {
            kind: MachineScalarKind::CoroutineFrameState,
            object: Value::Temp(object),
            offset: 16,
            value: Value::MachineScalar(MachineScalarValue::CoroutineFrameState(
                scoop_lir::CoroutineFrameState::Initial,
            )),
        },
        Instruction::MachineHeapLoad {
            out: state,
            kind: MachineScalarKind::CoroutineFrameState,
            object: Value::Temp(object),
            offset: 16,
        },
    ]);
    keep_heap_object_live_for_appended_access(function, object);

    let ir = ir_of(&module);
    assert!(
        ir.contains("machine_field_ptr")
            && ir
                .lines()
                .any(|line| line.contains("store i64 0") && line.contains("ptr addrspace(1)"))
            && ir
                .lines()
                .any(|line| line.contains("load i64") && line.contains("machine_field_ptr")),
        "typed machine heap access must lower only at the LLVM boundary:\n{ir}"
    );
}

#[test]
fn machine_heap_store_rejects_cross_domain_state() {
    let mut module = heap_module();
    let function = &mut module.functions[1];
    let object = function.temps.iter().next().expect("allocated object").0;
    function.blocks[function.entry]
        .instructions
        .push(Instruction::MachineHeapStore {
            kind: MachineScalarKind::CoroutineFrameState,
            object: Value::Temp(object),
            offset: 16,
            value: Value::MachineScalar(MachineScalarValue::CoroutineAdapterState(
                CoroutineAdapterState::Waiting,
            )),
        });
    keep_heap_object_live_for_appended_access(function, object);

    let machine = host_target_machine().expect("target machine");
    let context = Context::create();
    let error = emit_llvm_module(&context, &module, &machine, host_profile())
        .expect_err("frame and adapter state domains must not be interchangeable");
    assert!(
        error.0.contains("machine_heap_store") && error.0.contains("CoroutineFrameState"),
        "unexpected error: {error}"
    );
}

#[test]
fn generic_heap_store_rejects_machine_scalar() {
    let mut module = heap_module();
    let function = &mut module.functions[1];
    let object = function.temps.iter().next().expect("allocated object").0;
    function.blocks[function.entry]
        .instructions
        .push(Instruction::HeapStore {
            object: Value::Temp(object),
            offset: 16,
            value: Value::MachineScalar(MachineScalarValue::CoroutineFrameState(
                scoop_lir::CoroutineFrameState::Initial,
            )),
        });
    keep_heap_object_live_for_appended_access(function, object);

    let machine = host_target_machine().expect("target machine");
    let context = Context::create();
    let error = emit_llvm_module(&context, &module, &machine, host_profile())
        .expect_err("generic heap storage must not erase a machine domain");
    assert!(
        error.0.contains("heap_store") && error.0.contains("machine<coroutine-frame-state>"),
        "unexpected error: {error}"
    );
}

#[test]
fn generic_heap_load_rejects_machine_scalar_result() {
    let mut module = heap_module();
    let function = &mut module.functions[1];
    let object = function.temps.iter().next().expect("allocated object").0;
    let out = function.temps.alloc(Temp {
        ty: LirType::MachineScalar(MachineScalarKind::CoroutineFrameState),
    });
    function.blocks[function.entry]
        .instructions
        .push(Instruction::HeapLoad {
            out,
            object: Value::Temp(object),
            offset: 16,
        });
    keep_heap_object_live_for_appended_access(function, object);

    let machine = host_target_machine().expect("target machine");
    let context = Context::create();
    let error = emit_llvm_module(&context, &module, &machine, host_profile())
        .expect_err("generic heap loads must not manufacture a machine domain");
    assert!(
        error.0.contains("heap_load") && error.0.contains("machine scalar"),
        "unexpected error: {error}"
    );
}
