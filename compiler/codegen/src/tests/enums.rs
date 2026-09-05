use super::*;

/// An M4-shaped module: a tagged enum with three variants (0/1/2
/// fields of different types) and a niche enum (two variants,
/// `Option<String>`-style), both exercised through EnumWrap /
/// EnumTag / EnumField.
pub(super) fn enum_module() -> Module {
    let mut globals = Arena::default();
    let trap_message = globals.alloc(Global {
        symbol: "scoop.trap.0".to_string(),
        address_kind: PointerKind::Raw,
        scan: RefScan::None,
        init: GlobalInit::CString("unwrap on None".to_string()),
    });
    let mut enums = Arena::default();
    // Dot/Circle share the pure-value slot at 8; Rect owns a
    // ref-bearing slot at 16, with its String at offset 24.
    let shape = enums.alloc(EnumDef {
        name: "Shape".to_string(),
        repr: EnumRepr::Tagged {
            variants: vec![
                EnumVariantRepr {
                    fields: vec![],
                    slot_offset: 8,
                    slot_size: 0,
                    slot_align: 1,
                    gc_free: true,
                },
                EnumVariantRepr {
                    fields: vec![EnumFieldRepr {
                        ty: LirType::I64,
                        offset: 8,
                    }],
                    slot_offset: 8,
                    slot_size: 8,
                    slot_align: 8,
                    gc_free: true,
                },
                EnumVariantRepr {
                    fields: vec![
                        EnumFieldRepr {
                            ty: LirType::I64,
                            offset: 16,
                        },
                        EnumFieldRepr {
                            ty: MANAGED_PTR,
                            offset: 24,
                        },
                    ],
                    slot_offset: 16,
                    slot_size: 16,
                    slot_align: 8,
                    gc_free: false,
                },
            ],
            size: 32,
            align: 8,
        },
        scan: RefScan::References(vec![24]),
    });
    // enum Option<String> { None, Some(String) } — niche pointer.
    let option = enums.alloc(EnumDef {
        name: "Option<String>".to_string(),
        repr: EnumRepr::Niche {
            kind: scoop_lir::NichePointerKind::Managed,
            payload_variant: 1,
        },
        scan: RefScan::References(vec![0]),
    });
    let shape_ty = LirType::Enum(shape);
    let option_ty = LirType::Enum(option);
    let enum_tag_ty = LirType::MachineScalar(MachineScalarKind::EnumTag);

    // fun @scoop.tagged(s: Shape, p: ptr) -> i64: all three enum
    // instructions on the tagged representation, including a local
    // of enum type (alloca + store + load).
    let mut tagged_locals = Arena::default();
    let s2 = tagged_locals.alloc(Local {
        name: "s2".to_string(),
        ty: shape_ty.clone(),
    });
    let mut tagged_temps = Arena::default();
    let t0 = tagged_temps.alloc(Temp {
        ty: shape_ty.clone(),
    }); // enum_wrap v0 ()
    let t1 = tagged_temps.alloc(Temp {
        ty: enum_tag_ty.clone(),
    }); // enum_tag t0
    let t2 = tagged_temps.alloc(Temp {
        ty: shape_ty.clone(),
    }); // enum_wrap v1 (7)
    let t3 = tagged_temps.alloc(Temp { ty: LirType::I64 }); // enum_field v1 f0 t2
    let t4 = tagged_temps.alloc(Temp {
        ty: shape_ty.clone(),
    }); // enum_wrap v2 (t3, p)
    let t5 = tagged_temps.alloc(Temp {
        ty: enum_tag_ty.clone(),
    }); // enum_tag s (param)
    let t6 = tagged_temps.alloc(Temp { ty: MANAGED_PTR }); // enum_field v2 f1 s2 (local)
    let t7 = tagged_temps.alloc(Temp { ty: LirType::I64 }); // enum_field v2 f0 t4
    let t8 = tagged_temps.alloc(Temp { ty: LirType::I64 }); // t3 + t7
    let mut tagged_blocks = Arena::default();
    let tagged_entry = tagged_blocks.alloc(BasicBlock {
        name: "entry".to_string(),
        instructions: vec![
            Instruction::EnumWrap {
                out: t0,
                enum_id: shape,
                variant: 0,
                fields: vec![],
            },
            Instruction::EnumTag {
                out: t1,
                enum_id: shape,
                operand: Value::Temp(t0),
            },
            Instruction::EnumWrap {
                out: t2,
                enum_id: shape,
                variant: 1,
                fields: vec![Value::IntConst(7)],
            },
            Instruction::EnumField {
                out: t3,
                enum_id: shape,
                variant: 1,
                index: 0,
                operand: Value::Temp(t2),
            },
            Instruction::EnumWrap {
                out: t4,
                enum_id: shape,
                variant: 2,
                fields: vec![Value::Temp(t3), Value::Param(1)],
            },
            Instruction::EnumTag {
                out: t5,
                enum_id: shape,
                operand: Value::Param(0),
            },
            Instruction::Store {
                local: s2,
                value: Value::Temp(t4),
            },
            Instruction::EnumField {
                out: t6,
                enum_id: shape,
                variant: 2,
                index: 1,
                operand: Value::Local(s2),
            },
            Instruction::EnumField {
                out: t7,
                enum_id: shape,
                variant: 2,
                index: 0,
                operand: Value::Temp(t4),
            },
            Instruction::BinOp {
                out: t8,
                op: BinOp::Add,
                lhs: Value::Temp(t3),
                rhs: Value::Temp(t7),
            },
        ],
        terminator: Terminator::Return {
            value: Some(Value::Temp(t8)),
        },
    });
    let tagged = Function {
        gc_effect: GcEffect::Managed,
        symbol: "scoop.tagged".to_string(),
        params: vec![shape_ty.clone(), MANAGED_PTR],
        return_ty: LirType::I64,
        call_targets: CallTargets::default(),
        locals: tagged_locals,
        temps: tagged_temps,
        blocks: tagged_blocks,
        entry: tagged_entry,
    };

    // fun @scoop.niche(o: Option<String>) -> i64: all three enum
    // instructions on the niche representation (null ↔ variant 0).
    let mut niche_locals = Arena::default();
    let o2 = niche_locals.alloc(Local {
        name: "o2".to_string(),
        ty: option_ty.clone(),
    });
    let mut niche_temps = Arena::default();
    let n0 = niche_temps.alloc(Temp {
        ty: enum_tag_ty.clone(),
    }); // enum_tag o (param)
    let n1 = niche_temps.alloc(Temp { ty: MANAGED_PTR }); // enum_field v1 f0 o
    let n2 = niche_temps.alloc(Temp {
        ty: option_ty.clone(),
    }); // enum_wrap v1 (n1)
    let n3 = niche_temps.alloc(Temp {
        ty: option_ty.clone(),
    }); // enum_wrap v0 () → null
    let n4 = niche_temps.alloc(Temp {
        ty: enum_tag_ty.clone(),
    }); // enum_tag n3
    let n5 = niche_temps.alloc(Temp {
        ty: enum_tag_ty.clone(),
    }); // enum_tag o2 (local)
    let mut niche_blocks = Arena::default();
    let niche_entry = niche_blocks.alloc(BasicBlock {
        name: "entry".to_string(),
        instructions: vec![
            Instruction::EnumTag {
                out: n0,
                enum_id: option,
                operand: Value::Param(0),
            },
            Instruction::EnumField {
                out: n1,
                enum_id: option,
                variant: 1,
                index: 0,
                operand: Value::Param(0),
            },
            Instruction::EnumWrap {
                out: n2,
                enum_id: option,
                variant: 1,
                fields: vec![Value::Temp(n1)],
            },
            Instruction::Store {
                local: o2,
                value: Value::Temp(n2),
            },
            Instruction::EnumWrap {
                out: n3,
                enum_id: option,
                variant: 0,
                fields: vec![],
            },
            Instruction::EnumTag {
                out: n4,
                enum_id: option,
                operand: Value::Temp(n3),
            },
            Instruction::EnumTag {
                out: n5,
                enum_id: option,
                operand: Value::Local(o2),
            },
        ],
        terminator: Terminator::Return {
            value: Some(Value::Temp(n5)),
        },
    });
    let niche = Function {
        gc_effect: GcEffect::Managed,
        symbol: "scoop.niche".to_string(),
        params: vec![option_ty.clone()],
        return_ty: enum_tag_ty.clone(),
        call_targets: CallTargets::default(),
        locals: niche_locals,
        temps: niche_temps,
        blocks: niche_blocks,
        entry: niche_entry,
    };

    // fun @scoop.trap_on_none(): the `!!`-on-None path — trap call
    // (noreturn) followed by unreachable.
    let mut trap_targets = CallTargets::default();
    let trap_site = void_site(
        &mut trap_targets,
        TestCallProtocol::NoGc {
            destination: no_gc_runtime(scoop_lir::NoGcRuntimeFunction::Trap),
        },
        vec![RAW_PTR],
        vec![Value::Global(trap_message)],
    );
    let mut trap_blocks = Arena::default();
    let trap_entry = trap_blocks.alloc(BasicBlock {
        name: "entry".to_string(),
        instructions: vec![Instruction::Call { site: trap_site }],
        terminator: Terminator::Unreachable,
    });
    let trap_on_none = Function {
        gc_effect: GcEffect::Managed,
        symbol: "scoop.trap_on_none".to_string(),
        params: vec![],
        return_ty: LirType::Void,
        call_targets: trap_targets,
        locals: Arena::default(),
        temps: Arena::default(),
        blocks: trap_blocks,
        entry: trap_entry,
    };

    // A tagged enum crossing a managed call boundary. The LLVM
    // statepoint pass cannot lower aggregate returns directly, so
    // codegen must use its hidden result-slot ABI here.
    let mut produce_temps = Arena::default();
    let produced = produce_temps.alloc(Temp {
        ty: shape_ty.clone(),
    });
    let mut produce_blocks = Arena::default();
    let produce_entry = produce_blocks.alloc(BasicBlock {
        name: "entry".to_string(),
        instructions: vec![Instruction::EnumWrap {
            out: produced,
            enum_id: shape,
            variant: 1,
            fields: vec![Value::IntConst(9)],
        }],
        terminator: Terminator::Return {
            value: Some(Value::Temp(produced)),
        },
    });
    let produce = Function {
        gc_effect: GcEffect::Managed,
        symbol: "scoop.produce_shape".to_string(),
        params: vec![],
        return_ty: shape_ty.clone(),
        call_targets: CallTargets::default(),
        locals: Arena::default(),
        temps: produce_temps,
        blocks: produce_blocks,
        entry: produce_entry,
    };
    let mut consume_locals = Arena::default();
    let received = consume_locals.alloc(Local {
        name: "received".to_string(),
        ty: shape_ty.clone(),
    });
    let mut consume_temps = Arena::default();
    let tag = consume_temps.alloc(Temp {
        ty: enum_tag_ty.clone(),
    });
    let mut consume_targets = CallTargets::default();
    let produce_site = indirect_result_site(
        &mut consume_targets,
        TestCallProtocol::Managed {
            safepoint: 1,
            destination: managed_local(3),
        },
        Vec::new(),
        (shape_ty.clone(), RefScan::References(vec![24])),
        received,
        Vec::new(),
    );
    let mut consume_blocks = Arena::default();
    let consume_entry = consume_blocks.alloc(BasicBlock {
        name: "entry".to_string(),
        instructions: vec![
            Instruction::Call { site: produce_site },
            Instruction::EnumTag {
                out: tag,
                enum_id: shape,
                operand: Value::Local(received),
            },
        ],
        terminator: Terminator::Return {
            value: Some(Value::Temp(tag)),
        },
    });
    let consume = Function {
        gc_effect: GcEffect::Managed,
        symbol: "scoop.consume_shape".to_string(),
        params: vec![],
        return_ty: enum_tag_ty.clone(),
        call_targets: consume_targets,
        locals: consume_locals,
        temps: consume_temps,
        blocks: consume_blocks,
        entry: consume_entry,
    };
    let mut indirect_locals = Arena::default();
    let indirect_received = indirect_locals.alloc(Local {
        name: "indirect_received".to_string(),
        ty: shape_ty.clone(),
    });
    let mut indirect_temps = Arena::default();
    let indirect_tag = indirect_temps.alloc(Temp {
        ty: enum_tag_ty.clone(),
    });
    let mut indirect_targets = CallTargets::default();
    let dispatch = dispatch_destination(&mut indirect_targets, Value::Param(0), 0);
    let indirect_site = indirect_result_site(
        &mut indirect_targets,
        TestCallProtocol::Managed {
            safepoint: 2,
            destination: dispatch,
        },
        Vec::new(),
        (shape_ty.clone(), RefScan::References(vec![24])),
        indirect_received,
        Vec::new(),
    );
    let mut indirect_blocks = Arena::default();
    let indirect_entry = indirect_blocks.alloc(BasicBlock {
        name: "entry".to_string(),
        instructions: vec![
            Instruction::Call {
                site: indirect_site,
            },
            Instruction::EnumTag {
                out: indirect_tag,
                enum_id: shape,
                operand: Value::Local(indirect_received),
            },
        ],
        terminator: Terminator::Return {
            value: Some(Value::Temp(indirect_tag)),
        },
    });
    let consume_indirect = Function {
        gc_effect: GcEffect::Managed,
        symbol: "scoop.consume_shape_indirect".to_string(),
        params: vec![METADATA_PTR],
        return_ty: enum_tag_ty,
        call_targets: indirect_targets,
        locals: indirect_locals,
        temps: indirect_temps,
        blocks: indirect_blocks,
        entry: indirect_entry,
    };

    Module {
        globals,
        initialization_units: Arena::default(),
        structs: Arena::default(),
        enums,
        extern_functions: Default::default(),
        native_globals: Arena::default(),
        native_global_bridges: Default::default(),
        callback_bridges: Arena::default(),
        foreign_callback_families: Arena::default(),
        foreign_callback_bridges: Arena::default(),
        functions: vec![
            tagged,
            niche,
            trap_on_none,
            produce,
            consume,
            consume_indirect,
        ],
        entry_symbol: "scoop.tagged".to_string(),
        meta: string_metadata(),
    }
}

#[test]
fn emits_m4_enums() {
    let module = enum_module();
    let ir = ir_of(&module);
    assert!(
        ir.contains("store { i64, [16 x i8], ptr addrspace(1) } zeroinitializer"),
        "tagged enum construction must zero every inactive ref slot:\n{ir}"
    );
    assert!(
        ir.contains("{ i64, [16 x i8], ptr addrspace(1) }"),
        "tagged enum GC slots must remain typed AS1 fields through SROA:\n{ir}"
    );
    assert!(
        ir.contains("getelementptr i8, ptr %enum_wrap") && ir.contains("i64 24"),
        "ref-bearing variant fields must use their assigned slot offsets:\n{ir}"
    );
    let output =
        std::env::temp_dir().join(format!("scoop_codegen_m4_test_{}.o", std::process::id()));
    // `emit_object` verifies the LLVM module before writing, so a
    // successful return means `module.verify()` passed.
    emit_object(&module, &output, host_profile()).expect("emit object");
    let len = std::fs::metadata(&output)
        .expect("object file exists")
        .len();
    assert!(len > 0, "object file is empty");
    std::fs::remove_file(&output).ok();
}

fn enum_codegen_error(module: &Module) -> CodegenError {
    let machine = host_target_machine().expect("target machine");
    let context = Context::create();
    emit_llvm_module(&context, module, &machine, host_profile())
        .expect_err("malformed enum LIR must be rejected")
}

#[test]
fn enum_wrap_rejects_machine_scalar_for_i64_field() {
    let mut module = enum_module();
    let function = &mut module.functions[0];
    let Instruction::EnumWrap { fields, .. } = &mut function.blocks[function.entry].instructions[2]
    else {
        panic!("enum fixture must wrap its one-field variant")
    };
    fields[0] = Value::MachineScalar(MachineScalarValue::EnumTag(7));

    let error = enum_codegen_error(&module);
    assert!(
        error.0.contains("enum_wrap")
            && error.0.contains("machine<enum-tag>")
            && error.0.contains("expected i64"),
        "unexpected error: {error}"
    );
}

#[test]
fn enum_field_rejects_i64_as_machine_scalar_result() {
    let mut module = enum_module();
    let function = &mut module.functions[0];
    let out = match &function.blocks[function.entry].instructions[3] {
        Instruction::EnumField { out, .. } => *out,
        _ => panic!("enum fixture must extract its one-field variant"),
    };
    function.temps[out].ty = LirType::MachineScalar(MachineScalarKind::EnumTag);

    let error = enum_codegen_error(&module);
    assert!(
        error.0.contains("enum_field")
            && error.0.contains("machine<enum-tag>")
            && error.0.contains("expected i64"),
        "unexpected error: {error}"
    );
}

#[test]
fn tagged_enum_metadata_rejects_recursive_machine_scalar_payload() {
    let mut module = enum_module();
    let (_, definition) = module.enums.iter_mut().next().expect("tagged enum");
    let EnumRepr::Tagged { variants, .. } = &mut definition.repr else {
        panic!("first enum must be tagged")
    };
    variants[1].fields[0].ty =
        LirType::Aggregate(vec![LirType::MachineScalar(MachineScalarKind::EnumTag)]);
    module.functions.clear();

    let error = enum_codegen_error(&module);
    assert!(
        error.0.contains("enum `Shape` payload") && error.0.contains("machine scalar"),
        "unexpected error: {error}"
    );
}

#[test]
fn niche_enum_wrap_rejects_machine_scalar_payload() {
    let mut module = enum_module();
    let function = &mut module.functions[1];
    let Instruction::EnumWrap { fields, .. } = &mut function.blocks[function.entry].instructions[2]
    else {
        panic!("niche fixture must wrap its payload variant")
    };
    fields[0] = Value::MachineScalar(MachineScalarValue::EnumTag(1));

    let error = enum_codegen_error(&module);
    assert!(
        error.0.contains("enum_wrap")
            && error.0.contains("niche payload")
            && error.0.contains("machine<enum-tag>"),
        "unexpected error: {error}"
    );
}

#[test]
fn exact_raw_and_code_niches_emit_through_all_enum_operations() {
    for kind in [
        scoop_lir::NichePointerKind::Raw,
        scoop_lir::NichePointerKind::Code,
    ] {
        let mut module = enum_module();
        let option = module.enums.iter().nth(1).expect("niche enum").0;
        module.enums[option].repr = EnumRepr::Niche {
            kind,
            payload_variant: 1,
        };
        module.enums[option].scan = RefScan::None;

        let function = &mut module.functions[1];
        let field = match &function.blocks[function.entry].instructions[1] {
            Instruction::EnumField { out, .. } => *out,
            _ => panic!("niche fixture must project its payload"),
        };
        function.temps[field].ty = LirType::Ptr(kind.pointer_kind());
        module.globals.alloc(Global {
            symbol: format!("qualified_{}_niche", kind.pointer_kind().dump()),
            address_kind: PointerKind::Raw,
            scan: RefScan::None,
            init: GlobalInit::Storage {
                ty: LirType::Enum(option),
                initializer: ConstantValue::EnumUnit {
                    enum_id: option,
                    variant: 0,
                },
                thread_local: false,
            },
        });

        let ir = ir_of(&module);
        assert!(
            ir.contains(&format!("@qualified_{}_niche", kind.pointer_kind().dump())),
            "{ir}"
        );
    }
}

#[test]
fn niche_enum_wrap_rejects_raw_code_provenance_crossing() {
    for (expected, actual) in [
        (scoop_lir::NichePointerKind::Raw, PointerKind::Code),
        (scoop_lir::NichePointerKind::Code, PointerKind::Raw),
    ] {
        let mut module = enum_module();
        let option = module.enums.iter().nth(1).expect("niche enum").0;
        module.enums[option].repr = EnumRepr::Niche {
            kind: expected,
            payload_variant: 1,
        };
        module.enums[option].scan = RefScan::None;

        let function = &mut module.functions[1];
        let out = match &function.blocks[function.entry].instructions[2] {
            Instruction::EnumWrap { out, .. } => *out,
            _ => panic!("niche fixture must wrap its payload variant"),
        };
        function.blocks[function.entry].instructions = vec![Instruction::EnumWrap {
            out,
            enum_id: option,
            variant: 1,
            fields: vec![Value::NullPointer(actual)],
        }];
        function.blocks[function.entry].terminator = Terminator::Unreachable;

        let error = enum_codegen_error(&module);
        assert!(
            error.0.contains("enum_wrap")
                && error.0.contains(&format!("ptr<{}>", actual.dump()))
                && error
                    .0
                    .contains(&format!("expected ptr<{}>", expected.pointer_kind().dump())),
            "unexpected error: {error}"
        );
    }
}

#[test]
fn niche_enum_field_rejects_raw_code_provenance_crossing() {
    let mut module = enum_module();
    let option = module.enums.iter().nth(1).expect("niche enum").0;
    module.enums[option].repr = EnumRepr::Niche {
        kind: scoop_lir::NichePointerKind::Code,
        payload_variant: 1,
    };
    module.enums[option].scan = RefScan::None;

    let function = &mut module.functions[1];
    let out = match &function.blocks[function.entry].instructions[1] {
        Instruction::EnumField { out, .. } => *out,
        _ => panic!("niche fixture must project its payload"),
    };
    function.temps[out].ty = RAW_PTR;
    function.blocks[function.entry].instructions = vec![Instruction::EnumField {
        out,
        enum_id: option,
        variant: 1,
        index: 0,
        operand: Value::Param(0),
    }];
    function.blocks[function.entry].terminator = Terminator::Unreachable;

    let error = enum_codegen_error(&module);
    assert!(
        error.0.contains("enum_field")
            && error.0.contains("produces ptr<raw>")
            && error.0.contains("expected ptr<code>"),
        "unexpected error: {error}"
    );
}

#[test]
fn niche_enum_null_constant_cannot_bypass_pointer_provenance() {
    let mut module = enum_module();
    let option = module.enums.iter().nth(1).expect("niche enum").0;
    module.enums[option].repr = EnumRepr::Niche {
        kind: scoop_lir::NichePointerKind::Raw,
        payload_variant: 1,
    };
    module.enums[option].scan = RefScan::None;
    module.globals.alloc(Global {
        symbol: "crossed_niche_null".to_string(),
        address_kind: PointerKind::Raw,
        scan: RefScan::None,
        init: GlobalInit::Storage {
            ty: LirType::Enum(option),
            initializer: ConstantValue::NullPointer(PointerKind::Code),
            thread_local: false,
        },
    });

    let error = enum_codegen_error(&module);
    assert!(
        error.0.contains("code null constant")
            && error
                .0
                .contains(&format!("storage type enum{}", option.into_raw())),
        "unexpected error: {error}"
    );
}

#[test]
fn niche_enum_scan_must_match_its_pointer_provenance() {
    for (kind, scan) in [
        (scoop_lir::NichePointerKind::Managed, RefScan::None),
        (
            scoop_lir::NichePointerKind::Raw,
            RefScan::References(vec![0]),
        ),
        (
            scoop_lir::NichePointerKind::Code,
            RefScan::References(vec![0]),
        ),
    ] {
        let mut module = enum_module();
        let option = module.enums.iter().nth(1).expect("niche enum").0;
        module.enums[option].repr = EnumRepr::Niche {
            kind,
            payload_variant: 1,
        };
        module.enums[option].scan = scan;

        let error = enum_codegen_error(&module);
        assert!(
            error.0.contains("incompatible scan") && error.0.contains(kind.pointer_kind().dump()),
            "unexpected error: {error}"
        );
    }
}
