use super::*;

/// An M6-shaped module: class TypeDescriptors (parent chain, ordinary
/// user-method vtables with no compiler-owned `Any` slots, one itable) and
/// indirect calls through a table pointer (vtable / itable dispatch shape,
/// impl spec 2.9).
fn classes_module() -> Module {
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
            gc_effect: GcEffect::Managed,
            symbol: symbol.to_string(),
            params: vec![MANAGED_PTR],
            return_ty: MANAGED_PTR,
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
        gc_effect: GcEffect::Managed,
        symbol: "scoop_main".to_string(),
        params: vec![METADATA_PTR, MANAGED_PTR],
        return_ty: MANAGED_PTR,
        call_targets,
        locals: Arena::default(),
        temps,
        blocks,
        entry,
    };

    let mut meta = string_metadata();
    let describable = meta.type_descriptors.alloc(TypeDescriptor {
        name: "Describable".to_string(),
        symbol: "scoop_td_Describable".to_string(),
        runtime_type_id: 2,
        size: 0,
        align: 8,
        scan: TypeDescriptorScan::Fixed(RefScan::None),
        parent: None,
        vtable: vec![],
        itables: vec![],
    });
    let shape = meta.type_descriptors.alloc(TypeDescriptor {
        name: "Shape".to_string(),
        symbol: "scoop_td_Shape".to_string(),
        runtime_type_id: 3,
        size: 24,
        align: 8,
        scan: TypeDescriptorScan::Fixed(RefScan::References(vec![16])),
        parent: None,
        vtable: vec![DispatchEntry {
            callable: CallableRef::Local(scoop_lir::LocalFunctionId::from_u32(0)),
        }],
        itables: vec![],
    });
    meta.type_descriptors.alloc(TypeDescriptor {
        name: "Point".to_string(),
        symbol: "scoop_td_Point".to_string(),
        runtime_type_id: 4,
        size: 32,
        align: 8,
        scan: TypeDescriptorScan::Fixed(RefScan::References(vec![16])),
        parent: Some(TypeDescriptorRef::Local(shape)),
        vtable: vec![DispatchEntry {
            callable: CallableRef::Local(scoop_lir::LocalFunctionId::from_u32(1)),
        }],
        itables: vec![ItableRecord {
            interface: TypeDescriptorRef::Local(describable),
            slots: vec![DispatchEntry {
                callable: CallableRef::Local(scoop_lir::LocalFunctionId::from_u32(1)),
            }],
        }],
    });

    Module {
        globals: Arena::default(),
        initialization_units: Arena::default(),
        structs: Arena::default(),
        enums: Arena::default(),
        extern_functions: Default::default(),
        native_globals: Arena::default(),
        native_global_bridges: Default::default(),
        callback_bridges: Arena::default(),
        foreign_callback_bridges: Arena::default(),
        functions: vec![describe("Shape.describe"), describe("Point.describe"), main],
        entry_symbol: "scoop_main".to_string(),
        meta,
    }
}

#[test]
fn emits_m6_type_descriptors_and_call_indirect() {
    let module = classes_module();
    let output =
        std::env::temp_dir().join(format!("scoop_codegen_m6_test_{}.o", std::process::id()));
    // `emit_object` verifies the LLVM module before writing, so a
    // successful return means `module.verify()` passed.
    emit_object(&module, &output, host_profile()).expect("emit object");
    let len = std::fs::metadata(&output)
        .expect("object file exists")
        .len();
    assert!(len > 0, "object file is empty");
    std::fs::remove_file(&output).ok();
}

/// An M6 heap-access module with a typed TypeDescriptor operand,
/// HeapStore field writes, HeapLoad reads (header, i64
/// field, ptr field, TD vtable pointer), and a `scoop_rt_box` call
/// with a by-value aggregate payload.
pub(super) fn heap_module() -> Module {
    let globals = Arena::default();
    let mut meta = string_metadata();
    let point_descriptor = meta.type_descriptors.alloc(TypeDescriptor {
        name: "Point".to_string(),
        symbol: "scoop_td_Point".to_string(),
        runtime_type_id: 2,
        size: 32,
        align: 8,
        scan: TypeDescriptorScan::Fixed(RefScan::References(vec![24])),
        parent: None,
        vtable: vec![DispatchEntry {
            callable: CallableRef::Local(scoop_lir::LocalFunctionId::from_u32(0)),
        }],
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
        gc_effect: GcEffect::Managed,
        symbol: "Point.describe".to_string(),
        params: vec![MANAGED_PTR],
        return_ty: MANAGED_PTR,
        call_targets: CallTargets::default(),
        locals: Arena::default(),
        temps: Arena::default(),
        blocks: describe_blocks,
        entry: describe_entry,
    };

    // fun @scoop_main() -> void (M9 16-byte header, fields at byte
    // offsets 16 and 24):
    //   t0 = scoop_rt_alloc(@scoop_td_Point, 32)  (typed TD operand)
    //   heap_store t0 +16, 42     (i64 field)
    //   heap_store t0 +24, t0     (ptr field)
    //   t1 = heap_load t0 +0 : ptr   (object header: the TD)
    //   t2 = heap_load t0 +16 : i64  (field 1)
    //   t3 = heap_load t0 +24 : ptr  (field 2)
    //   t4 = heap_load t1 +40 : ptr  (TD field 5: the vtable pointer)
    //   t5 = aggregate (t2) : {i64}
    //   t6 = scoop_rt_box(@scoop_td_Point, t5, 8, none)  (by-value payload)
    //   t7 = scoop_rt_is_instance(t6, @scoop_td_Point) : i1
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
    let t8 = temps.alloc(Temp { ty: RAW_PTR });
    let mut locals = Arena::default();
    let payload = locals.alloc(Local {
        name: "box_payload".to_string(),
        ty: LirType::Aggregate(vec![LirType::I64]),
    });
    let mut call_targets = CallTargets::default();
    let alloc_site = direct_site(
        &mut call_targets,
        TestCallProtocol::Managed {
            safepoint: 1,
            destination: managed_runtime(scoop_lir::ManagedRuntimeFunction::Alloc),
        },
        vec![METADATA_PTR, LirType::I64],
        (MANAGED_PTR, RefScan::References(vec![0])),
        t0,
        vec![Value::TypeDescriptor(point_descriptor), Value::IntConst(32)],
    );
    let payload_scan = call_targets.root_scans.alloc(RefScan::None);
    let mut box_site = direct_site(
        &mut call_targets,
        TestCallProtocol::Managed {
            safepoint: 2,
            destination: managed_runtime(scoop_lir::ManagedRuntimeFunction::Box),
        },
        vec![METADATA_PTR, RAW_PTR, LirType::I64, METADATA_PTR],
        (MANAGED_PTR, RefScan::References(vec![0])),
        t6,
        vec![
            Value::TypeDescriptor(point_descriptor),
            Value::Temp(t8),
            Value::IntConst(8),
            Value::RootScan(payload_scan),
        ],
    );
    set_managed_live(
        &mut box_site,
        statepoint_live(vec![statepoint_value(
            scoop_lir::CallerRootSource::Temp(t3),
            MANAGED_PTR,
            &[0],
        )]),
    );
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
                value: Value::IntConst(42),
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
            Instruction::LocalAddress {
                out: t8,
                local: payload,
            },
            Instruction::Call { site: box_site },
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
        gc_effect: GcEffect::Managed,
        symbol: "scoop_main".to_string(),
        params: vec![],
        return_ty: LirType::Void,
        call_targets,
        locals,
        temps,
        blocks,
        entry,
    };

    Module {
        globals,
        initialization_units: Arena::default(),
        structs: Arena::default(),
        enums: Arena::default(),
        extern_functions: Default::default(),
        native_globals: Arena::default(),
        native_global_bridges: Default::default(),
        callback_bridges: Arena::default(),
        foreign_callback_bridges: Arena::default(),
        functions: vec![describe, main],
        entry_symbol: "scoop_main".to_string(),
        meta,
    }
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
        ir.contains("and i64 %tlab_cursor_int, -128")
            && ir.contains("add i64 %tlab_line_base, 128"),
        "the inline allocator must use runtime's 128-byte Immix line boundary:\n{ir}"
    );
    let output = std::env::temp_dir().join(format!(
        "scoop_codegen_m6_heap_test_{}.o",
        std::process::id()
    ));
    // `emit_object` verifies the LLVM module before writing, so a
    // successful return means `module.verify()` passed.
    emit_object(&module, &output, host_profile()).expect("emit object");
    let len = std::fs::metadata(&output)
        .expect("object file exists")
        .len();
    assert!(len > 0, "object file is empty");
    std::fs::remove_file(&output).ok();
}
