use la_arena::Arena;
use scoop_lir::{
    BasicBlock, CallDestination, CallEffect, CallSite, CallTargets, DirectCallSignature,
    DirectCallTarget, DispatchKind, DispatchSlot, EnumDef, EnumFieldRepr, EnumRepr,
    EnumVariantRepr, Global, GlobalInit, IndirectResultCallSignature, IndirectResultCallTarget,
    ItableRecord, Layout, LayoutKind, LirMeta, Local, MANAGED_PTR, METADATA_PTR, PointerKind,
    RAW_PTR, ResultStorage, Temp, TypeDescriptor, TypeDescriptorRef, TypeDescriptorScan,
    VoidCallSignature, VoidCallTarget, WellKnownLayouts, WellKnownTypeDescriptors,
};

use super::*;

fn void_site(
    targets: &mut CallTargets,
    destination: CallDestination,
    effect: CallEffect,
    params: Vec<LirType>,
    args: Vec<Value>,
) -> CallSite {
    let signature = targets.void_signatures.alloc(VoidCallSignature {
        params,
        calling_convention: scoop_lir::CallingConvention::Cdecl,
    });
    let target = targets.void_targets.alloc(VoidCallTarget {
        destination,
        signature,
        effect,
    });
    CallSite::Void { target, args }
}

fn direct_site(
    targets: &mut CallTargets,
    destination: CallDestination,
    effect: CallEffect,
    params: Vec<LirType>,
    result: (LirType, RefScan),
    out: TempId,
    args: Vec<Value>,
) -> CallSite {
    let signature = targets.direct_signatures.alloc(DirectCallSignature {
        params,
        result: result.0,
        result_scan: result.1,
        calling_convention: scoop_lir::CallingConvention::Cdecl,
    });
    let target = targets.direct_targets.alloc(DirectCallTarget {
        destination,
        signature,
        effect,
    });
    CallSite::Direct { target, out, args }
}

fn indirect_result_site(
    targets: &mut CallTargets,
    destination: CallDestination,
    effect: CallEffect,
    params: Vec<LirType>,
    result: (LirType, RefScan),
    storage: scoop_lir::LocalId,
    args: Vec<Value>,
) -> CallSite {
    let signature = targets
        .indirect_result_signatures
        .alloc(IndirectResultCallSignature {
            params,
            result: ResultStorage {
                ty: result.0,
                scan: result.1,
            },
            calling_convention: scoop_lir::CallingConvention::Cdecl,
        });
    let target = targets
        .indirect_result_targets
        .alloc(IndirectResultCallTarget {
            destination,
            signature,
            effect,
        });
    CallSite::IndirectResult {
        target,
        storage,
        args,
    }
}

fn dispatch_destination(targets: &mut CallTargets, table: Value, index: u32) -> CallDestination {
    let slot = targets.dispatch_slots.alloc(DispatchSlot {
        kind: DispatchKind::Virtual,
        index,
    });
    CallDestination::Dispatch { table, slot }
}

fn string_metadata() -> LirMeta {
    let mut layouts = Arena::new();
    let string_layout = layouts.alloc(Layout {
        name: "String".to_string(),
        size: 24,
        align: 8,
        fields: Vec::new(),
        c_layout: None,
        interior_mutable: false,
        kind: LayoutKind::Intrinsic(scoop_lir::IntrinsicTypeRepresentation::String),
    });
    let mut type_descriptors = Arena::new();
    let string_descriptor = type_descriptors.alloc(TypeDescriptor {
        name: "String".to_string(),
        symbol: scoop_lir::STRING_TD_SYMBOL.to_string(),
        runtime_type_id: 1,
        size: 24,
        align: 8,
        scan: TypeDescriptorScan::Fixed(RefScan::None),
        parent: None,
        vtable: Vec::new(),
        itables: Vec::new(),
    });
    LirMeta {
        well_known_layouts: WellKnownLayouts {
            string: string_layout,
        },
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

fn array_type(
    meta: &mut LirMeta,
    name: &str,
    kind: scoop_lir::ArrayKind,
    element: LirType,
    element_size: u64,
    element_align: u64,
    scan: RefScan,
) -> ArrayTypeId {
    let runtime_type_id = meta.type_descriptors.len() as u64 + 1;
    let type_descriptor = meta.type_descriptors.alloc(TypeDescriptor {
        name: name.to_string(),
        symbol: format!("scoop_td_{name}"),
        runtime_type_id,
        size: element_size,
        align: element_align,
        scan: TypeDescriptorScan::ArrayElement {
            stride: element_size,
            scan,
        },
        parent: None,
        vtable: Vec::new(),
        itables: Vec::new(),
    });
    meta.arrays.alloc(ArrayType {
        kind,
        element,
        element_size,
        element_align,
        type_descriptor: TypeDescriptorRef::Local(type_descriptor),
    })
}

/// An M2-shaped module: string constants, a user function exercising
/// alloca/store/load, arithmetic, branches, aggregates and
/// extractvalue, plus calls into the new runtime functions.
fn values_module() -> Module {
    let mut globals = Arena::default();
    let hello = globals.alloc(Global {
        symbol: "scoop.string.0".to_string(),
        address_kind: PointerKind::Managed,
        init: GlobalInit::StringConst("hello, ".to_string()),
    });
    let world = globals.alloc(Global {
        symbol: "scoop.string.1".to_string(),
        address_kind: PointerKind::Managed,
        init: GlobalInit::StringConst("world".to_string()),
    });

    let mut locals = Arena::default();
    let n = locals.alloc(Local {
        name: "n".to_string(),
        ty: LirType::I64,
    });
    let point = locals.alloc(Local {
        name: "p".to_string(),
        ty: LirType::Aggregate(vec![LirType::I64, LirType::I64]),
    });
    let unit = locals.alloc(Local {
        name: "u".to_string(),
        ty: LirType::Aggregate(vec![]),
    });

    let mut temps = Arena::default();
    let temp = |temps: &mut Arena<Temp>, ty: LirType| temps.alloc(Temp { ty });
    // t0 = 1 + 2
    let t0 = temp(&mut temps, LirType::I64);
    // t1 = {40, 2} (Point)
    let t1 = temp(
        &mut temps,
        LirType::Aggregate(vec![LirType::I64, LirType::I64]),
    );
    // t2 = p.x
    let t2 = temp(&mut temps, LirType::I64);
    // t3 = t2 < 100
    let t3 = temp(&mut temps, LirType::I1);
    // t4 = -t2
    let t4 = temp(&mut temps, LirType::I64);
    // t5 = !true
    let t5 = temp(&mut temps, LirType::I1);
    // t6 = concat(hello, world)
    let t6 = temp(&mut temps, MANAGED_PTR);
    // t7 = ()
    let t7 = temp(&mut temps, LirType::Aggregate(vec![]));
    let mut call_targets = CallTargets::default();
    let concat = direct_site(
        &mut call_targets,
        CallDestination::Runtime(scoop_lir::RuntimeFunction::StringConcat),
        CallEffect::ManagedSafepoint,
        vec![MANAGED_PTR, MANAGED_PTR],
        (MANAGED_PTR, RefScan::References(vec![0])),
        t6,
        vec![Value::Global(hello), Value::Global(world)],
    );

    // Allocate the four blocks first so terminators can reference
    // them, then fill in their bodies.
    let mut blocks = Arena::default();
    let placeholder = |blocks: &mut Arena<BasicBlock>, name: &str| {
        blocks.alloc(BasicBlock {
            name: name.to_string(),
            instructions: vec![],
            terminator: Terminator::Return { value: None },
        })
    };
    let entry = placeholder(&mut blocks, "entry");
    let then_block = placeholder(&mut blocks, "then");
    let else_block = placeholder(&mut blocks, "else");
    let end = placeholder(&mut blocks, "end");

    blocks[entry] = BasicBlock {
        name: "entry".to_string(),
        instructions: vec![
            Instruction::BinOp {
                out: t0,
                op: BinOp::Add,
                lhs: Value::IntConst(1),
                rhs: Value::IntConst(2),
            },
            Instruction::Store {
                local: n,
                value: Value::Temp(t0),
            },
            Instruction::MakeAggregate {
                out: t1,
                elements: vec![Value::IntConst(40), Value::IntConst(2)],
            },
            Instruction::Store {
                local: point,
                value: Value::Temp(t1),
            },
            Instruction::ExtractValue {
                out: t2,
                aggregate: Value::Local(point),
                index: 0,
            },
            Instruction::BinOp {
                out: t3,
                op: BinOp::Lt,
                lhs: Value::Temp(t2),
                rhs: Value::IntConst(100),
            },
            Instruction::Call { site: concat },
        ],
        terminator: Terminator::CondBr {
            cond: Value::Temp(t3),
            then_block,
            else_block,
        },
    };
    blocks[then_block] = BasicBlock {
        name: "then".to_string(),
        instructions: vec![Instruction::UnaryOp {
            out: t4,
            op: UnOp::Neg,
            operand: Value::Temp(t2),
        }],
        terminator: Terminator::Br(end),
    };
    blocks[else_block] = BasicBlock {
        name: "else".to_string(),
        instructions: vec![Instruction::UnaryOp {
            out: t5,
            op: UnOp::Not,
            operand: Value::BoolConst(true),
        }],
        terminator: Terminator::Br(end),
    };
    blocks[end] = BasicBlock {
        name: "end".to_string(),
        instructions: vec![
            Instruction::MakeAggregate {
                out: t7,
                elements: vec![],
            },
            Instruction::Store {
                local: unit,
                value: Value::Temp(t7),
            },
        ],
        terminator: Terminator::Return { value: None },
    };

    Module {
        globals,
        structs: Arena::default(),
        enums: Arena::default(),
        extern_functions: Arena::default(),
        native_globals: Arena::default(),
        native_global_bridges: Default::default(),
        callback_bridges: Arena::default(),
        foreign_callback_bridges: Arena::default(),
        functions: vec![Function {
            gc_effect: GcEffect::Managed,
            symbol: "scoop_main".to_string(),
            params: vec![],
            return_ty: LirType::Void,
            call_targets,
            locals,
            temps,
            blocks,
            entry,
        }],
        entry_symbol: "scoop_main".to_string(),
        meta: string_metadata(),
    }
}

#[test]
fn emits_non_empty_object_file() {
    let module = values_module();
    let output = std::env::temp_dir().join(format!("scoop_codegen_test_{}.o", std::process::id()));
    emit_object(&module, &output).expect("emit object");
    let len = std::fs::metadata(&output)
        .expect("object file exists")
        .len();
    assert!(len > 0, "object file is empty");
    std::fs::remove_file(&output).ok();
}

/// An M4-shaped module: a tagged enum with three variants (0/1/2
/// fields of different types) and a niche enum (two variants,
/// `Option<String>`-style), both exercised through EnumWrap /
/// EnumTag / EnumField.
fn enum_module() -> Module {
    let mut globals = Arena::default();
    let trap_message = globals.alloc(Global {
        symbol: "scoop.trap.0".to_string(),
        address_kind: PointerKind::Raw,
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
        repr: EnumRepr::Niche { payload_variant: 1 },
        scan: RefScan::References(vec![0]),
    });
    let shape_ty = LirType::Enum(shape);
    let option_ty = LirType::Enum(option);

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
    let t1 = tagged_temps.alloc(Temp { ty: LirType::I64 }); // enum_tag t0
    let t2 = tagged_temps.alloc(Temp {
        ty: shape_ty.clone(),
    }); // enum_wrap v1 (7)
    let t3 = tagged_temps.alloc(Temp { ty: LirType::I64 }); // enum_field v1 f0 t2
    let t4 = tagged_temps.alloc(Temp {
        ty: shape_ty.clone(),
    }); // enum_wrap v2 (t3, p)
    let t5 = tagged_temps.alloc(Temp { ty: LirType::I64 }); // enum_tag s (param)
    let t6 = tagged_temps.alloc(Temp { ty: MANAGED_PTR }); // enum_field v2 f1 s2 (local)
    let t7 = tagged_temps.alloc(Temp { ty: LirType::I64 }); // enum_field v2 f0 t4
    let t8 = tagged_temps.alloc(Temp { ty: LirType::I64 }); // t1 + t3
    let t9 = tagged_temps.alloc(Temp { ty: LirType::I64 }); // t5 + t7
    let t10 = tagged_temps.alloc(Temp { ty: LirType::I64 }); // t8 + t9
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
                lhs: Value::Temp(t1),
                rhs: Value::Temp(t3),
            },
            Instruction::BinOp {
                out: t9,
                op: BinOp::Add,
                lhs: Value::Temp(t5),
                rhs: Value::Temp(t7),
            },
            Instruction::BinOp {
                out: t10,
                op: BinOp::Add,
                lhs: Value::Temp(t8),
                rhs: Value::Temp(t9),
            },
        ],
        terminator: Terminator::Return {
            value: Some(Value::Temp(t10)),
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
    let n0 = niche_temps.alloc(Temp { ty: LirType::I64 }); // enum_tag o (param)
    let n1 = niche_temps.alloc(Temp { ty: MANAGED_PTR }); // enum_field v1 f0 o
    let n2 = niche_temps.alloc(Temp {
        ty: option_ty.clone(),
    }); // enum_wrap v1 (n1)
    let n3 = niche_temps.alloc(Temp {
        ty: option_ty.clone(),
    }); // enum_wrap v0 () → null
    let n4 = niche_temps.alloc(Temp { ty: LirType::I64 }); // enum_tag n3
    let n5 = niche_temps.alloc(Temp { ty: LirType::I64 }); // enum_tag o2 (local)
    let n6 = niche_temps.alloc(Temp { ty: LirType::I64 }); // n0 + n4
    let n7 = niche_temps.alloc(Temp { ty: LirType::I64 }); // n6 + n5
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
            Instruction::BinOp {
                out: n6,
                op: BinOp::Add,
                lhs: Value::Temp(n0),
                rhs: Value::Temp(n4),
            },
            Instruction::BinOp {
                out: n7,
                op: BinOp::Add,
                lhs: Value::Temp(n6),
                rhs: Value::Temp(n5),
            },
        ],
        terminator: Terminator::Return {
            value: Some(Value::Temp(n7)),
        },
    });
    let niche = Function {
        gc_effect: GcEffect::Managed,
        symbol: "scoop.niche".to_string(),
        params: vec![option_ty.clone()],
        return_ty: LirType::I64,
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
        CallDestination::Runtime(scoop_lir::RuntimeFunction::Trap),
        CallEffect::NoGc,
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
    let tag = consume_temps.alloc(Temp { ty: LirType::I64 });
    let mut consume_targets = CallTargets::default();
    let produce_site = indirect_result_site(
        &mut consume_targets,
        CallDestination::Local(scoop_lir::LocalFunctionId::from_u32(3)),
        CallEffect::ManagedSafepoint,
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
        return_ty: LirType::I64,
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
    let indirect_tag = indirect_temps.alloc(Temp { ty: LirType::I64 });
    let mut indirect_targets = CallTargets::default();
    let dispatch = dispatch_destination(&mut indirect_targets, Value::Param(0), 0);
    let indirect_site = indirect_result_site(
        &mut indirect_targets,
        dispatch,
        CallEffect::ManagedSafepoint,
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
        return_ty: LirType::I64,
        call_targets: indirect_targets,
        locals: indirect_locals,
        temps: indirect_temps,
        blocks: indirect_blocks,
        entry: indirect_entry,
    };

    Module {
        globals,
        structs: Arena::default(),
        enums,
        extern_functions: Arena::default(),
        native_globals: Arena::default(),
        native_global_bridges: Default::default(),
        callback_bridges: Arena::default(),
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
        ir.contains("store { i64, [24 x i8] } zeroinitializer"),
        "tagged enum construction must zero every inactive ref slot:\n{ir}"
    );
    assert!(
        ir.contains("getelementptr i8, ptr %enum_wrap") && ir.contains("i64 24"),
        "ref-bearing variant fields must use their assigned slot offsets:\n{ir}"
    );
    let output =
        std::env::temp_dir().join(format!("scoop_codegen_m4_test_{}.o", std::process::id()));
    // `emit_object` verifies the LLVM module before writing, so a
    // successful return means `module.verify()` passed.
    emit_object(&module, &output).expect("emit object");
    let len = std::fs::metadata(&output)
        .expect("object file exists")
        .len();
    assert!(len > 0, "object file is empty");
    std::fs::remove_file(&output).ok();
}

/// An M5-shaped module: ArrayAlloc with i64 and aggregate (Point)
/// elements, ArrayLen, bounds-checked ArrayGet / ArraySet, and
/// ArrayClone on both element shapes.
fn arrays_module() -> Module {
    let point = LirType::Aggregate(vec![LirType::I64, LirType::I64]);
    let mut meta = string_metadata();
    let int_array = array_type(
        &mut meta,
        "Array<Int>",
        scoop_lir::ArrayKind::Immutable,
        LirType::I64,
        8,
        8,
        RefScan::None,
    );
    let mutable_int_array = array_type(
        &mut meta,
        "MutableArray<Int>",
        scoop_lir::ArrayKind::Mutable,
        LirType::I64,
        8,
        8,
        RefScan::None,
    );
    let point_array = array_type(
        &mut meta,
        "Array<Point>",
        scoop_lir::ArrayKind::Immutable,
        point.clone(),
        16,
        8,
        RefScan::None,
    );
    let mutable_point_array = array_type(
        &mut meta,
        "MutableArray<Point>",
        scoop_lir::ArrayKind::Mutable,
        point.clone(),
        16,
        8,
        RefScan::None,
    );

    let mut locals = Arena::default();
    let numbers = locals.alloc(Local {
        name: "numbers".to_string(),
        ty: MANAGED_PTR,
    });

    let mut temps = Arena::default();
    let t0 = temps.alloc(Temp { ty: MANAGED_PTR }); // array_alloc (1, 2, 3)
    let t1 = temps.alloc(Temp { ty: LirType::I64 }); // array_len t0
    let t2 = temps.alloc(Temp { ty: LirType::I64 }); // array_get t0[1]
    let t3 = temps.alloc(Temp { ty: MANAGED_PTR }); // array_clone t0
    let t4 = temps.alloc(Temp { ty: point.clone() }); // aggregate (t2, t1)
    let t5 = temps.alloc(Temp { ty: MANAGED_PTR }); // array_alloc (t4, t4)
    let t6 = temps.alloc(Temp { ty: point.clone() }); // array_get t5[1]
    let t7 = temps.alloc(Temp { ty: LirType::I64 }); // extract t6.1
    let t8 = temps.alloc(Temp { ty: MANAGED_PTR }); // array_clone t5
    let t9 = temps.alloc(Temp { ty: LirType::I64 }); // t1 + t7

    let mut blocks = Arena::default();
    let entry = blocks.alloc(BasicBlock {
        name: "entry".to_string(),
        instructions: vec![
            Instruction::ArrayAlloc {
                out: t0,
                elements: vec![Value::IntConst(1), Value::IntConst(2), Value::IntConst(3)],
                array_type: int_array,
            },
            Instruction::Store {
                local: numbers,
                value: Value::Temp(t0),
            },
            Instruction::ArrayLen {
                out: t1,
                operand: Value::Local(numbers),
                array_type: int_array,
            },
            Instruction::ArrayGet {
                out: t2,
                array: Value::Local(numbers),
                index: Value::IntConst(1),
                array_type: int_array,
            },
            Instruction::ArraySet {
                array: Value::Local(numbers),
                index: Value::IntConst(0),
                value: Value::Temp(t2),
                array_type: int_array,
            },
            Instruction::ArrayClone {
                out: t3,
                operand: Value::Local(numbers),
                array_type: mutable_int_array,
            },
            Instruction::MakeAggregate {
                out: t4,
                elements: vec![Value::Temp(t2), Value::Temp(t1)],
            },
            Instruction::ArrayAlloc {
                out: t5,
                elements: vec![Value::Temp(t4), Value::Temp(t4)],
                array_type: point_array,
            },
            Instruction::ArrayGet {
                out: t6,
                array: Value::Temp(t5),
                index: Value::IntConst(0),
                array_type: point_array,
            },
            Instruction::ExtractValue {
                out: t7,
                aggregate: Value::Temp(t6),
                index: 1,
            },
            Instruction::ArraySet {
                array: Value::Temp(t5),
                index: Value::Temp(t1),
                value: Value::Temp(t6),
                array_type: point_array,
            },
            Instruction::ArrayClone {
                out: t8,
                operand: Value::Temp(t5),
                array_type: mutable_point_array,
            },
            Instruction::BinOp {
                out: t9,
                op: BinOp::Add,
                lhs: Value::Temp(t1),
                rhs: Value::Temp(t7),
            },
            Instruction::ArraySet {
                array: Value::Temp(t3),
                index: Value::IntConst(0),
                value: Value::IntConst(0),
                array_type: mutable_int_array,
            },
            Instruction::ArraySet {
                array: Value::Temp(t8),
                index: Value::IntConst(0),
                value: Value::Temp(t6),
                array_type: mutable_point_array,
            },
        ],
        terminator: Terminator::Return { value: None },
    });

    Module {
        globals: Arena::default(),
        structs: Arena::default(),
        enums: Arena::default(),
        extern_functions: Arena::default(),
        native_globals: Arena::default(),
        native_global_bridges: Default::default(),
        callback_bridges: Arena::default(),
        foreign_callback_bridges: Arena::default(),
        functions: vec![Function {
            gc_effect: GcEffect::Managed,
            symbol: "scoop_main".to_string(),
            params: vec![],
            return_ty: LirType::Void,
            call_targets: CallTargets::default(),
            locals,
            temps,
            blocks,
            entry,
        }],
        entry_symbol: "scoop_main".to_string(),
        meta,
    }
}

#[test]
fn emits_m5_arrays() {
    let module = arrays_module();
    let ir = ir_of(&module);
    assert!(
        ir.lines().any(|line| {
            line.contains("call ptr @scoop_rt_array_clone")
                && line.contains("scoop_td_MutableArray<Int>")
        }),
        "Int clone must receive the target nominal descriptor:\n{ir}"
    );
    assert!(
        ir.lines().any(|line| {
            line.contains("call ptr @scoop_rt_array_clone")
                && line.contains("scoop_td_MutableArray<Point>")
        }),
        "Point clone must receive the target nominal descriptor:\n{ir}"
    );
    let output =
        std::env::temp_dir().join(format!("scoop_codegen_m5_test_{}.o", std::process::id()));
    // `emit_object` verifies the LLVM module before writing, so a
    // successful return means `module.verify()` passed.
    emit_object(&module, &output).expect("emit object");
    let len = std::fs::metadata(&output)
        .expect("object file exists")
        .len();
    assert!(len > 0, "object file is empty");
    std::fs::remove_file(&output).ok();
}

/// An M6-shaped module: class TypeDescriptors (parent chain, vtable
/// with the three Any default slots plus a user method, one itable)
/// and indirect calls through a table pointer (vtable / itable
/// dispatch shape, impl spec 2.9).
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
    let result_call = direct_site(
        &mut call_targets,
        result_dispatch,
        CallEffect::ManagedSafepoint,
        vec![MANAGED_PTR],
        (MANAGED_PTR, RefScan::References(vec![0])),
        t0,
        vec![Value::Param(1)],
    );
    let void_dispatch = dispatch_destination(&mut call_targets, Value::Param(0), 1);
    let void_call = void_site(
        &mut call_targets,
        void_dispatch,
        CallEffect::ManagedSafepoint,
        vec![MANAGED_PTR],
        vec![Value::Param(1)],
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
        structs: Arena::default(),
        enums: Arena::default(),
        extern_functions: Arena::default(),
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
    emit_object(&module, &output).expect("emit object");
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
fn heap_module() -> Module {
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
    //   t6 = scoop_rt_box(@scoop_td_Point, t5, 8)  (by-value payload)
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
        CallDestination::Runtime(scoop_lir::RuntimeFunction::Alloc),
        CallEffect::ManagedSafepoint,
        vec![METADATA_PTR, LirType::I64],
        (MANAGED_PTR, RefScan::References(vec![0])),
        t0,
        vec![Value::TypeDescriptor(point_descriptor), Value::IntConst(32)],
    );
    let box_site = direct_site(
        &mut call_targets,
        CallDestination::Runtime(scoop_lir::RuntimeFunction::Box),
        CallEffect::ManagedSafepoint,
        vec![METADATA_PTR, RAW_PTR, LirType::I64],
        (MANAGED_PTR, RefScan::References(vec![0])),
        t6,
        vec![
            Value::TypeDescriptor(point_descriptor),
            Value::Temp(t8),
            Value::IntConst(8),
        ],
    );
    let is_instance_site = direct_site(
        &mut call_targets,
        CallDestination::Runtime(scoop_lir::RuntimeFunction::IsInstance),
        CallEffect::NoGc,
        vec![MANAGED_PTR, METADATA_PTR],
        (LirType::I1, RefScan::None),
        t7,
        vec![Value::Temp(t6), Value::TypeDescriptor(point_descriptor)],
    );
    let dispatch = dispatch_destination(&mut call_targets, Value::Temp(t4), 0);
    let dispatch_site = void_site(
        &mut call_targets,
        dispatch,
        CallEffect::ManagedSafepoint,
        vec![MANAGED_PTR],
        vec![Value::Temp(t3)],
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
        structs: Arena::default(),
        enums: Arena::default(),
        extern_functions: Arena::default(),
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
    emit_object(&module, &output).expect("emit object");
    let len = std::fs::metadata(&output)
        .expect("object file exists")
        .len();
    assert!(len > 0, "object file is empty");
    std::fs::remove_file(&output).ok();
}

/// An M8-shaped module (runtime spec 5): Invoke / InvokeIndirect
/// sharing one catch-all landing pad, plus the cleanup-pad /
/// EndCatch / Resume shape used for exceptional handler exits.
fn exceptions_module() -> Module {
    // fun @scoop.thrower(e: ptr) -> void: the rethrow shape — Throw
    // as the last instruction; the Unreachable terminator emits the
    // LLVM `unreachable` after the noreturn runtime call.
    let mut thrower_blocks = Arena::default();
    let thrower_entry = thrower_blocks.alloc(BasicBlock {
        name: "entry".to_string(),
        instructions: vec![Instruction::Throw {
            exception: Value::Param(0),
        }],
        terminator: Terminator::Unreachable,
    });
    let thrower = Function {
        gc_effect: GcEffect::Managed,
        symbol: "scoop.thrower".to_string(),
        params: vec![MANAGED_PTR],
        return_ty: LirType::Void,
        call_targets: CallTargets::default(),
        locals: Arena::default(),
        temps: Arena::default(),
        blocks: thrower_blocks,
        entry: thrower_entry,
    };

    let mut may_throw_blocks = Arena::default();
    let may_throw_entry = may_throw_blocks.alloc(BasicBlock {
        name: "entry".to_string(),
        instructions: Vec::new(),
        terminator: Terminator::Return {
            value: Some(Value::IntConst(1)),
        },
    });
    let may_throw = Function {
        gc_effect: GcEffect::Managed,
        symbol: "scoop.may_throw".to_string(),
        params: Vec::new(),
        return_ty: LirType::I64,
        call_targets: CallTargets::default(),
        locals: Arena::default(),
        temps: Arena::default(),
        blocks: may_throw_blocks,
        entry: may_throw_entry,
    };

    // fun @scoop.eh_test(table: ptr) -> i64:
    //   entry:  t0 = invoke_indirect table[0]() normal @normal unwind @lpad
    //   normal: t1 = invoke @scoop.may_throw() normal @done unwind @lpad
    //   done:   t2 = t0 + t1; ret t2
    //   lpad:   t3 = landingpad : ptr
    //           end_catch
    //           ret 0
    //   cleanup: t4 = cleanup_pad; end_catch; resume t4
    let mut temps = Arena::default();
    let t0 = temps.alloc(Temp { ty: LirType::I64 });
    let t1 = temps.alloc(Temp { ty: LirType::I64 });
    let t2 = temps.alloc(Temp { ty: LirType::I64 });
    let t3 = temps.alloc(Temp {
        ty: LirType::ExceptionRecord,
    });
    let t4 = temps.alloc(Temp { ty: RAW_PTR });
    let t5 = temps.alloc(Temp { ty: MANAGED_PTR });
    let t6 = temps.alloc(Temp {
        ty: LirType::ExceptionRecord,
    });
    let t7 = temps.alloc(Temp { ty: RAW_PTR });
    let mut blocks = Arena::default();
    let placeholder = |blocks: &mut Arena<BasicBlock>, name: &str| {
        blocks.alloc(BasicBlock {
            name: name.to_string(),
            instructions: vec![],
            terminator: Terminator::Unreachable,
        })
    };
    let entry = placeholder(&mut blocks, "entry");
    let normal = placeholder(&mut blocks, "normal");
    let done = placeholder(&mut blocks, "done");
    let lpad = placeholder(&mut blocks, "lpad");
    let cleanup = placeholder(&mut blocks, "cleanup");
    let mut call_targets = CallTargets::default();
    let dispatch = dispatch_destination(&mut call_targets, Value::Param(0), 0);
    let first_invoke = direct_site(
        &mut call_targets,
        dispatch,
        CallEffect::ManagedSafepoint,
        Vec::new(),
        (LirType::I64, RefScan::None),
        t0,
        Vec::new(),
    );
    let second_invoke = direct_site(
        &mut call_targets,
        CallDestination::Local(scoop_lir::LocalFunctionId::from_u32(1)),
        CallEffect::ManagedSafepoint,
        Vec::new(),
        (LirType::I64, RefScan::None),
        t1,
        Vec::new(),
    );
    blocks[entry] = BasicBlock {
        name: "entry".to_string(),
        instructions: vec![Instruction::Invoke {
            site: first_invoke,
            normal,
            unwind: lpad,
        }],
        terminator: Terminator::Br(normal),
    };
    blocks[normal] = BasicBlock {
        name: "normal".to_string(),
        instructions: vec![Instruction::Invoke {
            site: second_invoke,
            normal: done,
            unwind: lpad,
        }],
        terminator: Terminator::Br(done),
    };
    blocks[done] = BasicBlock {
        name: "done".to_string(),
        instructions: vec![Instruction::BinOp {
            out: t2,
            op: BinOp::Add,
            lhs: Value::Temp(t0),
            rhs: Value::Temp(t1),
        }],
        terminator: Terminator::Return {
            value: Some(Value::Temp(t2)),
        },
    };
    blocks[lpad] = BasicBlock {
        name: "lpad".to_string(),
        instructions: vec![
            Instruction::LandingPad {
                record: t3,
                raw: t4,
            },
            Instruction::BeginCatch {
                out: t5,
                raw: Value::Temp(t4),
            },
            Instruction::EndCatch,
        ],
        terminator: Terminator::Return {
            value: Some(Value::IntConst(0)),
        },
    };
    blocks[cleanup] = BasicBlock {
        name: "cleanup".to_string(),
        instructions: vec![
            Instruction::CleanupPad {
                record: t6,
                raw: t7,
            },
            Instruction::EndCatch,
        ],
        terminator: Terminator::Resume {
            exception: Value::Temp(t6),
        },
    };
    let eh_test = Function {
        gc_effect: GcEffect::Managed,
        symbol: "scoop.eh_test".to_string(),
        params: vec![METADATA_PTR],
        return_ty: LirType::I64,
        call_targets,
        locals: Arena::default(),
        temps,
        blocks,
        entry,
    };

    Module {
        globals: Arena::default(),
        structs: Arena::default(),
        enums: Arena::default(),
        extern_functions: Arena::default(),
        native_globals: Arena::default(),
        native_global_bridges: Default::default(),
        callback_bridges: Arena::default(),
        foreign_callback_bridges: Arena::default(),
        functions: vec![thrower, may_throw, eh_test],
        entry_symbol: "scoop.eh_test".to_string(),
        meta: string_metadata(),
    }
}

#[test]
fn emits_m8_exceptions() {
    let module = exceptions_module();
    let ir = ir_of(&module);
    assert!(ir.contains("@__cxa_begin_catch"));
    assert!(ir.contains("@__cxa_end_catch"));
    assert!(ir.contains("resume { ptr, i32 }"));
    let output =
        std::env::temp_dir().join(format!("scoop_codegen_m8_test_{}.o", std::process::id()));
    // `emit_object` verifies the LLVM module before writing, so a
    // successful return means `module.verify()` passed. M9: this
    // also proves invoke / landingpad and the GC strategy coexist
    // — every function carries `gc "statepoint-example"` and the
    // module goes through `rewrite-statepoints-for-gc`.
    emit_object(&module, &output).expect("emit object");
    let bytes = std::fs::read(&output).expect("read object");
    assert!(!bytes.is_empty(), "object file is empty");
    // The landing pad function must carry an exception table.
    let text = String::from_utf8_lossy(&bytes);
    assert!(
        text.contains("gcc_except_tab"),
        "object file lacks exception tables"
    );
    assert!(
        text.contains("__llvm_stackmaps"),
        "object file lacks the __llvm_stackmaps section"
    );
    std::fs::remove_file(&output).ok();
}

// ---- M9: GC support ----

/// The LLVM IR text of a module, verified, before the statepoint
/// rewrite.
fn ir_of(module: &Module) -> String {
    let machine = host_target_machine().expect("target machine");
    let context = Context::create();
    let llvm = emit_llvm_module(&context, module, &machine).expect("emit module");
    llvm.verify().expect("valid LLVM module");
    llvm.print_to_string().to_string()
}

#[test]
fn typed_intrinsic_string_supplies_the_only_descriptor_definition() {
    let ir = ir_of(&values_module());
    assert_eq!(
        ir.match_indices("@scoop_td_String =").count(),
        1,
        "String must have exactly one descriptor definition"
    );
    assert!(ir.contains("@scoop_td_String ="));
}

#[test]
fn typed_local_call_signature_cannot_be_replaced_by_a_callsite_guess() {
    let mut module = exceptions_module();
    module.functions[1].params.push(LirType::I64);
    let machine = host_target_machine().expect("target machine");
    let context = Context::create();
    let error = emit_llvm_module(&context, &module, &machine)
        .expect_err("the local declaration and typed call target disagree");
    assert!(
        error.0.contains("disagrees with its existing declaration"),
        "unexpected error: {error}"
    );
}

#[test]
fn typed_no_gc_effect_keeps_the_call_outside_statepoints() {
    let module = heap_module();
    let machine = host_target_machine().expect("target machine");
    let context = Context::create();
    let llvm = emit_llvm_module(&context, &module, &machine).expect("emit module");
    llvm.verify().expect("valid LLVM module");
    llvm.run_passes(
        "rewrite-statepoints-for-gc",
        &machine,
        PassBuilderOptions::create(),
    )
    .expect("rewrite-statepoints-for-gc pass");
    let rewritten = llvm.print_to_string().to_string();
    assert!(
        rewritten
            .lines()
            .any(|line| line.contains("call i1 @scoop_rt_is_instance")),
        "NoGC call must remain an ordinary call after statepoint rewriting:\n{rewritten}"
    );
}

#[test]
fn native_calls_publish_roots_transition_and_reload() {
    let mut extern_functions = Arena::default();
    let c_call = extern_functions.alloc(ExternFunction {
        source_name: "wait".to_string(),
        native_symbol: "native_wait".to_string(),
        library: "fixture".to_string(),
        calling_convention: scoop_lir::CallingConvention::Cdecl,
        params: Vec::new(),
        return_type: LirType::Void,
        kind: ExternFunctionKind::C {
            bridge_symbol: "scoop_c_bridge_wait".to_string(),
            params: Vec::new(),
            return_type: scoop_lir::CType::Unit,
        },
    });
    let borrowed = extern_functions.alloc(ExternFunction {
        source_name: "borrowed".to_string(),
        native_symbol: "native_borrowed".to_string(),
        library: "fixture".to_string(),
        calling_convention: scoop_lir::CallingConvention::Cdecl,
        params: Vec::new(),
        return_type: MANAGED_PTR,
        kind: ExternFunctionKind::Scoop {
            gc_effect: GcEffect::Managed,
        },
    });

    let mut safe_targets = CallTargets::default();
    let safe_site = void_site(
        &mut safe_targets,
        CallDestination::Extern(c_call),
        CallEffect::NativeSafe,
        Vec::new(),
        Vec::new(),
    );
    let mut safe_blocks = Arena::default();
    let safe_entry = safe_blocks.alloc(BasicBlock {
        name: "entry".to_string(),
        instructions: vec![Instruction::NativeCall {
            site: safe_site,
            roots: vec![scoop_lir::CallerRoot {
                source: scoop_lir::CallerRootSource::Param(0),
                scan: scoop_lir::NonEmptyRefScan::new(RefScan::References(vec![0])).unwrap(),
            }],
        }],
        terminator: Terminator::Return {
            value: Some(Value::Param(0)),
        },
    });
    let safe = Function {
        gc_effect: GcEffect::Managed,
        symbol: "safe_root".to_string(),
        params: vec![MANAGED_PTR],
        return_ty: MANAGED_PTR,
        call_targets: safe_targets,
        locals: Arena::default(),
        temps: Arena::default(),
        blocks: safe_blocks,
        entry: safe_entry,
    };

    let mut borrowed_temps = Arena::default();
    let result = borrowed_temps.alloc(Temp { ty: MANAGED_PTR });
    let mut borrowed_targets = CallTargets::default();
    let borrowed_site = direct_site(
        &mut borrowed_targets,
        CallDestination::Extern(borrowed),
        CallEffect::NativeBorrowed,
        Vec::new(),
        (MANAGED_PTR, RefScan::References(vec![0])),
        result,
        Vec::new(),
    );
    let mut borrowed_blocks = Arena::default();
    let borrowed_entry = borrowed_blocks.alloc(BasicBlock {
        name: "entry".to_string(),
        instructions: vec![Instruction::NativeCall {
            site: borrowed_site,
            roots: Vec::new(),
        }],
        terminator: Terminator::Return {
            value: Some(Value::Temp(result)),
        },
    });
    let borrowed_function = Function {
        gc_effect: GcEffect::Managed,
        symbol: "borrowed_result".to_string(),
        params: Vec::new(),
        return_ty: MANAGED_PTR,
        call_targets: borrowed_targets,
        locals: Arena::default(),
        temps: borrowed_temps,
        blocks: borrowed_blocks,
        entry: borrowed_entry,
    };

    let module = Module {
        globals: Arena::default(),
        structs: Arena::default(),
        enums: Arena::default(),
        extern_functions,
        native_globals: Arena::default(),
        native_global_bridges: Default::default(),
        callback_bridges: Arena::default(),
        foreign_callback_bridges: Arena::default(),
        functions: vec![safe, borrowed_function],
        entry_symbol: "safe_root".to_string(),
        meta: string_metadata(),
    };

    let ir = ir_of(&module);
    assert!(ir.contains("@scoop_rt_push_caller_roots"));
    assert!(ir.contains("@scoop_rt_enter_native_safe"));
    assert!(ir.contains("@scoop_rt_leave_native_safe"));
    assert!(ir.contains("@scoop_rt_enter_native_borrowed"));
    assert!(ir.contains("@scoop_rt_leave_native_borrowed"));
    assert!(ir.contains("@scoop_rt_pop_caller_roots"));
    assert!(
        ir.contains("store ptr null, ptr %native_result"),
        "managed native result storage must be zero before publication:\n{ir}"
    );
    assert!(
        ir.contains("%caller_root_reload = load ptr, ptr %caller_root_param"),
        "published parameter roots must be reloaded after leave-native:\n{ir}"
    );
}

#[test]
fn continuation_state_atomics_keep_their_llvm_orderings() {
    let mut temps = Arena::default();
    let loaded = temps.alloc(Temp { ty: LirType::I64 });
    let observed = temps.alloc(Temp { ty: LirType::I64 });
    let mut blocks = Arena::default();
    let entry = blocks.alloc(BasicBlock {
        name: "entry".to_string(),
        instructions: vec![
            Instruction::AtomicLoad {
                out: loaded,
                object: Value::Param(0),
                offset: 16,
            },
            Instruction::AtomicStore {
                object: Value::Param(0),
                offset: 16,
                value: Value::IntConst(2),
            },
            Instruction::AtomicCompareExchange {
                out: observed,
                object: Value::Param(0),
                offset: 16,
                expected: Value::Temp(loaded),
                replacement: Value::IntConst(6),
            },
        ],
        terminator: Terminator::Return {
            value: Some(Value::Temp(observed)),
        },
    });
    let module = Module {
        globals: Arena::default(),
        structs: Arena::default(),
        enums: Arena::default(),
        extern_functions: Arena::default(),
        native_globals: Arena::default(),
        native_global_bridges: Default::default(),
        callback_bridges: Arena::default(),
        foreign_callback_bridges: Arena::default(),
        functions: vec![Function {
            gc_effect: GcEffect::Managed,
            symbol: "continuation_atomics".to_string(),
            params: vec![MANAGED_PTR],
            return_ty: LirType::I64,
            call_targets: CallTargets::default(),
            locals: Arena::default(),
            temps,
            blocks,
            entry,
        }],
        entry_symbol: "continuation_atomics".to_string(),
        meta: string_metadata(),
    };

    let ir = ir_of(&module);
    assert!(
        ir.contains("load atomic i64, ptr %atomic_field_ptr acquire"),
        "continuation state reads must be acquire loads:\n{ir}"
    );
    assert!(
        ir.contains("store atomic i64 2, ptr %atomic_field_ptr1 release"),
        "continuation state publication must be a release store:\n{ir}"
    );
    assert!(
        ir.contains("cmpxchg ptr %atomic_field_ptr2") && ir.contains("acq_rel acquire"),
        "continuation state claims must be acq_rel/acquire compare-exchange:\n{ir}"
    );
}

/// An M11-shaped module with both ordinary and suspend closure calls.
/// Both return aggregates so the machine ABI has a leading result slot;
/// the closure remains the first source-level argument and the suspend
/// call carries its continuation immediately after it.
fn closure_abi_module() -> Module {
    let ordinary_result_ty = LirType::Aggregate(vec![LirType::I64, MANAGED_PTR]);
    let suspend_result_ty = LirType::Aggregate(vec![LirType::I64, LirType::I64]);
    let mut locals = Arena::default();
    let ordinary_result = locals.alloc(Local {
        name: "ordinary_result".to_string(),
        ty: ordinary_result_ty.clone(),
    });
    let suspend_result = locals.alloc(Local {
        name: "suspend_result".to_string(),
        ty: suspend_result_ty.clone(),
    });
    let mut call_targets = CallTargets::default();
    let ordinary_dispatch = dispatch_destination(&mut call_targets, Value::Param(0), 2);
    let ordinary_site = indirect_result_site(
        &mut call_targets,
        ordinary_dispatch,
        CallEffect::ManagedSafepoint,
        vec![MANAGED_PTR, LirType::I64],
        (ordinary_result_ty, RefScan::References(vec![8])),
        ordinary_result,
        vec![Value::Param(0), Value::Param(1)],
    );
    let suspend_dispatch = dispatch_destination(&mut call_targets, Value::Param(0), 2);
    let suspend_site = indirect_result_site(
        &mut call_targets,
        suspend_dispatch,
        CallEffect::ManagedSafepoint,
        vec![MANAGED_PTR, MANAGED_PTR],
        (suspend_result_ty, RefScan::None),
        suspend_result,
        vec![Value::Param(0), Value::Param(2)],
    );
    let mut blocks = Arena::default();
    let entry = blocks.alloc(BasicBlock {
        name: "entry".to_string(),
        instructions: vec![
            Instruction::Call {
                site: ordinary_site,
            },
            Instruction::Call { site: suspend_site },
        ],
        terminator: Terminator::Return { value: None },
    });

    Module {
        globals: Arena::default(),
        structs: Arena::default(),
        enums: Arena::default(),
        extern_functions: Arena::default(),
        native_globals: Arena::default(),
        native_global_bridges: Default::default(),
        callback_bridges: Arena::default(),
        foreign_callback_bridges: Arena::default(),
        functions: vec![Function {
            gc_effect: GcEffect::Managed,
            symbol: "scoop.closure_abi".to_string(),
            params: vec![MANAGED_PTR, LirType::I64, MANAGED_PTR],
            return_ty: LirType::Void,
            call_targets,
            locals,
            temps: Arena::default(),
            blocks,
            entry,
        }],
        entry_symbol: "scoop.closure_abi".to_string(),
        meta: string_metadata(),
    }
}

#[test]
fn closure_calls_preserve_hidden_abi_and_indirect_statepoints() {
    let module = closure_abi_module();
    let ir = ir_of(&module);
    assert_eq!(
        ir.matches("getelementptr ptr, ptr %0, i32 2").count(),
        2,
        "closure calls must load invoke from slot 2:\n{ir}"
    );
    assert!(
        ir.lines().any(|line| {
            line.contains("call void %dispatch_function")
                && line.contains("(ptr %ordinary_result, ptr %0, i64 %1)")
        }),
        "ordinary closure ABI must be (result slot, closure, arguments):\n{ir}"
    );
    assert!(
        ir.lines().any(|line| {
            line.contains("call void %dispatch_function")
                && line.contains("ptr %suspend_result, ptr %0, ptr %2")
        }),
        "suspend closure ABI must keep continuation after the closure:\n{ir}"
    );
    assert!(
        ir.contains("%ordinary_result = alloca { i64, ptr }")
            && ir.contains("%suspend_result = alloca { i64, i64 }"),
        "aggregate closure results must use typed return storage:\n{ir}"
    );

    let machine = host_target_machine().expect("target machine");
    let context = Context::create();
    let llvm = emit_llvm_module(&context, &module, &machine).expect("emit module");
    llvm.verify().expect("valid LLVM module");
    llvm.run_passes(
        "rewrite-statepoints-for-gc",
        &machine,
        PassBuilderOptions::create(),
    )
    .expect("rewrite-statepoints-for-gc pass");
    let rewritten = llvm.print_to_string().to_string();
    assert_eq!(
        rewritten
            .lines()
            .filter(|line| {
                line.contains("call token")
                    && line.contains("gc.statepoint")
                    && line.contains("%dispatch_function")
            })
            .count(),
        2,
        "both managed indirect closure calls must become statepoints:\n{rewritten}"
    );
}

/// An M9-shaped module: a HeapStore and an ArraySet (both carry
/// the write-barrier card mark) in the entry block, then a
/// `while`-shaped loop (header ← body back edge) for the loop
/// safepoint poll.
fn barrier_module() -> Module {
    let mut meta = string_metadata();
    let int_array = array_type(
        &mut meta,
        "Array<Int>",
        scoop_lir::ArrayKind::Immutable,
        LirType::I64,
        8,
        8,
        RefScan::None,
    );
    let mut temps = Arena::default();
    let t0 = temps.alloc(Temp { ty: MANAGED_PTR }); // alloc result
    let mut call_targets = CallTargets::default();
    let alloc_site = direct_site(
        &mut call_targets,
        CallDestination::Runtime(scoop_lir::RuntimeFunction::Alloc),
        CallEffect::ManagedSafepoint,
        vec![METADATA_PTR, LirType::I64],
        (MANAGED_PTR, RefScan::References(vec![0])),
        t0,
        vec![Value::Param(0), Value::IntConst(24)],
    );
    let mut blocks = Arena::default();
    let entry = blocks.alloc(BasicBlock {
        name: "entry".to_string(),
        instructions: vec![],
        terminator: Terminator::Unreachable, // placeholder, filled below
    });
    let header = blocks.alloc(BasicBlock {
        name: "while.cond".to_string(),
        instructions: vec![],
        terminator: Terminator::Unreachable,
    });
    let body = blocks.alloc(BasicBlock {
        name: "while.body".to_string(),
        instructions: vec![],
        terminator: Terminator::Unreachable,
    });
    let exit = blocks.alloc(BasicBlock {
        name: "while.exit".to_string(),
        instructions: vec![],
        terminator: Terminator::Return { value: None },
    });
    blocks[entry] = BasicBlock {
        name: "entry".to_string(),
        instructions: vec![
            Instruction::Call { site: alloc_site },
            Instruction::HeapStore {
                object: Value::Temp(t0),
                offset: 16,
                value: Value::IntConst(42),
            },
            Instruction::ArraySet {
                array: Value::Param(1),
                index: Value::IntConst(0),
                value: Value::IntConst(7),
                array_type: int_array,
            },
        ],
        terminator: Terminator::Br(header),
    };
    blocks[header] = BasicBlock {
        name: "while.cond".to_string(),
        instructions: vec![],
        terminator: Terminator::CondBr {
            cond: Value::BoolConst(true),
            then_block: body,
            else_block: exit,
        },
    };
    blocks[body] = BasicBlock {
        name: "while.body".to_string(),
        instructions: vec![],
        terminator: Terminator::Br(header),
    };
    Module {
        globals: Arena::default(),
        structs: Arena::default(),
        enums: Arena::default(),
        extern_functions: Arena::default(),
        native_globals: Arena::default(),
        native_global_bridges: Default::default(),
        callback_bridges: Arena::default(),
        foreign_callback_bridges: Arena::default(),
        functions: vec![Function {
            gc_effect: GcEffect::Managed,
            symbol: "scoop_main".to_string(),
            params: vec![METADATA_PTR, MANAGED_PTR],
            return_ty: LirType::Void,
            call_targets,
            locals: Arena::default(),
            temps,
            blocks,
            entry,
        }],
        entry_symbol: "scoop_main".to_string(),
        meta,
    }
}

#[test]
fn functions_carry_the_gc_strategy_and_poll_safepoints() {
    let ir = ir_of(&barrier_module());
    assert!(
        ir.contains("gc \"statepoint-example\""),
        "function lacks the GC strategy:\n{ir}"
    );
    // One poll at the function entry, one at the loop header.
    let polls = ir.matches("call void @scoop_rt_safepoint()").count();
    assert_eq!(polls, 2, "entry + loop-header safepoint polls:\n{ir}");
}

#[test]
fn no_gc_functions_carry_neither_gc_strategy_nor_safepoint_polls() {
    let mut module = barrier_module();
    module.functions[0].gc_effect = GcEffect::NoGc;
    let ir = ir_of(&module);
    assert!(
        !ir.contains("gc \"statepoint-example\""),
        "NoGC function unexpectedly carries the GC strategy:\n{ir}"
    );
    assert_eq!(
        ir.matches("call void @scoop_rt_safepoint()").count(),
        0,
        "NoGC function unexpectedly polls safepoints:\n{ir}"
    );
}

#[test]
fn heap_stores_mark_the_write_barrier_card() {
    let ir = ir_of(&barrier_module());
    // The card table is a pointer variable: load the (pre-biased)
    // base, then GEP by the card index.
    assert!(
        ir.contains("@scoop_gc_card_table = external global ptr"),
        "card table pointer global missing:\n{ir}"
    );
    assert!(
        ir.contains("load ptr, ptr @scoop_gc_card_table"),
        "card table base load missing:\n{ir}"
    );
    assert!(ir.contains("lshr i64"), "card index shift missing:\n{ir}");
    // One monotonic atomic card mark per heap store: the HeapStore and
    // the ArraySet element store.
    let marks = ir.matches(" = atomicrmw or ptr ").count();
    assert_eq!(marks, 2, "one card mark per heap store:\n{ir}");
}

#[test]
fn heap_store_inside_the_object_header_is_rejected() {
    let mut module = barrier_module();
    let function = &mut module.functions[0];
    let entry = function.entry;
    function.blocks[entry].instructions[1] = Instruction::HeapStore {
        object: Value::IntConst(0),
        offset: 8,
        value: Value::IntConst(42),
    };
    let machine = host_target_machine().expect("target machine");
    let context = Context::create();
    let error = emit_llvm_module(&context, &module, &machine)
        .expect_err("offset 8 is inside the object header, not a field");
    assert!(
        error.0.contains("object header"),
        "unexpected error: {error}"
    );
}

#[test]
fn statepoints_and_stackmaps_are_emitted() {
    let module = values_module();
    let machine = host_target_machine().expect("target machine");
    let context = Context::create();
    let llvm = emit_llvm_module(&context, &module, &machine).expect("emit module");
    llvm.verify().expect("valid LLVM module");
    // The same pass `emit_object` runs before writing the object
    // (the M0 spike's shape).
    llvm.run_passes(
        "rewrite-statepoints-for-gc",
        &machine,
        PassBuilderOptions::create(),
    )
    .expect("rewrite-statepoints-for-gc pass");
    let ir = llvm.print_to_string().to_string();
    assert!(
        ir.contains("gc.statepoint"),
        "statepoint intrinsics missing after rewrite-statepoints-for-gc:\n{ir}"
    );

    let output =
        std::env::temp_dir().join(format!("scoop_codegen_m9_test_{}.o", std::process::id()));
    machine
        .write_to_file(&llvm, FileType::Object, &output)
        .expect("write object");
    let bytes = std::fs::read(&output).expect("read object");
    let text = String::from_utf8_lossy(&bytes);
    assert!(
        text.contains("__llvm_stackmaps"),
        "object file lacks the __llvm_stackmaps section"
    );
    std::fs::remove_file(&output).ok();
}

#[test]
fn type_descriptors_carry_the_gc_scan_descriptors() {
    // A class's plain table is count-prefixed (`[N, off0, ..]`,
    // runtime/include/scoop_rt.h's M9 scan-descriptor contract).
    let ir = ir_of(&heap_module());
    assert!(
        ir.contains("@scoop_td_Point.refs = private constant [2 x i64] [i64 1, i64 24]"),
        "plain scan table must be count-prefixed:\n{ir}"
    );

    // A reference-element array's TD carries the SCOOP_REFS_ARRAY
    // sentinel (u64::MAX, printed -1), its stride, and a pointer
    // to the recursive scan for one inline element.
    let nested_element_scan = RefScan::Sequence(vec![
        RefScan::References(vec![16]),
        RefScan::References(vec![8]),
    ]);
    let mut meta = string_metadata();
    let ref_array_type = array_type(
        &mut meta,
        "ArrayRef",
        scoop_lir::ArrayKind::Immutable,
        MANAGED_PTR,
        8,
        8,
        RefScan::References(vec![0]),
    );
    let nested_array_type = array_type(
        &mut meta,
        "ArrayNested",
        scoop_lir::ArrayKind::Immutable,
        LirType::Aggregate(vec![LirType::I64, MANAGED_PTR, MANAGED_PTR]),
        24,
        8,
        nested_element_scan,
    );
    let mut temps = Arena::default();
    let array = temps.alloc(Temp { ty: MANAGED_PTR });
    let nested_array = temps.alloc(Temp { ty: MANAGED_PTR });
    let mut blocks = Arena::default();
    let entry = blocks.alloc(BasicBlock {
        name: "entry".to_string(),
        instructions: vec![
            Instruction::ArrayAlloc {
                out: array,
                elements: vec![],
                array_type: ref_array_type,
            },
            Instruction::ArrayAlloc {
                out: nested_array,
                elements: vec![],
                array_type: nested_array_type,
            },
        ],
        terminator: Terminator::Return { value: None },
    });
    let runtime_type_id = meta.type_descriptors.len() as u64 + 1;
    meta.type_descriptors.alloc(TypeDescriptor {
        name: "Holder".to_string(),
        symbol: "scoop_td_Holder".to_string(),
        runtime_type_id,
        size: 56,
        align: 8,
        scan: TypeDescriptorScan::Fixed(RefScan::Sequence(vec![
            RefScan::References(vec![16]),
            RefScan::References(vec![40, 48]),
        ])),
        parent: None,
        vtable: vec![],
        itables: vec![],
    });
    let module = Module {
        globals: Arena::default(),
        structs: Arena::default(),
        enums: Arena::default(),
        extern_functions: Arena::default(),
        native_globals: Arena::default(),
        native_global_bridges: Default::default(),
        callback_bridges: Arena::default(),
        foreign_callback_bridges: Arena::default(),
        functions: vec![Function {
            gc_effect: GcEffect::Managed,
            symbol: "scoop_main".to_string(),
            params: vec![],
            return_ty: LirType::Void,
            call_targets: CallTargets::default(),
            locals: Arena::default(),
            temps,
            blocks,
            entry,
        }],
        entry_symbol: "scoop_main".to_string(),
        meta,
    };
    let ir = ir_of(&module);
    assert!(
            ir.contains(
                "@scoop_td_ArrayRef.element = private constant [2 x i64] [i64 1, i64 0]"
            ) && ir.contains(
                "@scoop_td_ArrayRef.refs = private constant [3 x i64] [i64 -1, i64 8, i64 ptrtoint (ptr @scoop_td_ArrayRef.element to i64)]"
            ),
            "reference-element array TD must carry SCOOP_REFS_ARRAY:\n{ir}"
        );
    assert!(
            ir.contains(
                "@scoop_td_ArrayNested.element.part.1 = private constant [2 x i64] [i64 1, i64 8]"
            ) && ir.contains(
                "@scoop_td_ArrayNested.element = private constant [4 x i64] [i64 -2, i64 2"
            ) && ir.contains(
                "@scoop_td_ArrayNested.refs = private constant [3 x i64] [i64 -1, i64 24, i64 ptrtoint (ptr @scoop_td_ArrayNested.element to i64)]"
            ),
            "aggregate array TD must wrap the recursive element scan:\n{ir}"
        );
    assert!(
        ir.contains(
            "@scoop_td_Holder.refs.part.1 = private constant [3 x i64] [i64 2, i64 40, i64 48]"
        ),
        "nested tagged enum scan must use fixed ref offsets:\n{ir}"
    );
    assert!(
        ir.contains("@scoop_td_Holder.refs = private constant [4 x i64] [i64 -2, i64 2"),
        "aggregate scan must compose fixed scans:\n{ir}"
    );
}

#[test]
fn c_layout_matches_llvm_and_generated_c_assertions() {
    let mut structs = Arena::default();
    let inner = structs.alloc(StructDef {
        name: "Inner".to_string(),
        fields: vec![
            scoop_lir::StructField {
                ty: LirType::I1,
                layout: scoop_lir::FieldLayout {
                    offset: 0,
                    access_align: 1,
                },
            },
            scoop_lir::StructField {
                ty: LirType::I64,
                layout: scoop_lir::FieldLayout {
                    offset: 1,
                    access_align: 1,
                },
            },
        ],
        size: 16,
        align: 8,
        c_layout: Some(scoop_lir::CLayout {
            aligned: 8,
            packed: 1,
        }),
        interior_mutable: false,
    });
    let outer = structs.alloc(StructDef {
        name: "Outer".to_string(),
        fields: vec![
            scoop_lir::StructField {
                ty: LirType::I1,
                layout: scoop_lir::FieldLayout {
                    offset: 0,
                    access_align: 1,
                },
            },
            scoop_lir::StructField {
                ty: LirType::Struct(inner),
                layout: scoop_lir::FieldLayout {
                    offset: 2,
                    access_align: 2,
                },
            },
            scoop_lir::StructField {
                ty: LirType::I64,
                layout: scoop_lir::FieldLayout {
                    offset: 18,
                    access_align: 2,
                },
            },
        ],
        size: 32,
        align: 16,
        c_layout: Some(scoop_lir::CLayout {
            aligned: 16,
            packed: 2,
        }),
        interior_mutable: true,
    });
    let mut enums = Arena::default();
    let wrapped = enums.alloc(EnumDef {
        name: "Wrapped".to_string(),
        repr: EnumRepr::Tagged {
            variants: vec![
                EnumVariantRepr {
                    fields: vec![EnumFieldRepr {
                        ty: LirType::Struct(outer),
                        offset: 16,
                    }],
                    slot_offset: 16,
                    slot_size: 32,
                    slot_align: 16,
                    gc_free: true,
                },
                EnumVariantRepr {
                    fields: Vec::new(),
                    slot_offset: 16,
                    slot_size: 0,
                    slot_align: 1,
                    gc_free: true,
                },
            ],
            size: 48,
            align: 16,
        },
        scan: RefScan::None,
    });

    let mut meta = string_metadata();
    let outer_array = array_type(
        &mut meta,
        "ArrayOuter",
        scoop_lir::ArrayKind::Immutable,
        LirType::Struct(outer),
        32,
        16,
        RefScan::None,
    );

    let mut temps = Arena::default();
    let inner_value = temps.alloc(Temp {
        ty: LirType::Struct(inner),
    });
    let inner_field = temps.alloc(Temp { ty: LirType::I64 });
    let outer_value = temps.alloc(Temp {
        ty: LirType::Struct(outer),
    });
    let outer_field = temps.alloc(Temp {
        ty: LirType::Struct(inner),
    });
    let array = temps.alloc(Temp { ty: MANAGED_PTR });
    let loaded = temps.alloc(Temp {
        ty: LirType::Struct(outer),
    });
    let mut blocks = Arena::default();
    let entry = blocks.alloc(BasicBlock {
        name: "entry".to_string(),
        instructions: vec![
            Instruction::MakeAggregate {
                out: inner_value,
                elements: vec![Value::BoolConst(true), Value::IntConst(7)],
            },
            Instruction::ExtractValue {
                out: inner_field,
                aggregate: Value::Temp(inner_value),
                index: 1,
            },
            Instruction::MakeAggregate {
                out: outer_value,
                elements: vec![
                    Value::BoolConst(false),
                    Value::Temp(inner_value),
                    Value::IntConst(9),
                ],
            },
            Instruction::ExtractValue {
                out: outer_field,
                aggregate: Value::Temp(outer_value),
                index: 1,
            },
            Instruction::ArrayAlloc {
                out: array,
                elements: vec![Value::Temp(outer_value)],
                array_type: outer_array,
            },
            Instruction::ArrayGet {
                out: loaded,
                array: Value::Temp(array),
                index: Value::IntConst(0),
                array_type: outer_array,
            },
        ],
        terminator: Terminator::Return { value: None },
    });
    let mut module = Module {
        globals: Arena::default(),
        structs,
        enums,
        extern_functions: Arena::default(),
        native_globals: Arena::default(),
        native_global_bridges: Default::default(),
        callback_bridges: Arena::default(),
        foreign_callback_bridges: Arena::default(),
        functions: vec![Function {
            gc_effect: GcEffect::Managed,
            symbol: "scoop_main".to_string(),
            params: vec![],
            return_ty: LirType::Void,
            call_targets: CallTargets::default(),
            locals: Arena::default(),
            temps,
            blocks,
            entry,
        }],
        entry_symbol: "scoop_main".to_string(),
        meta,
    };

    let machine = host_target_machine().expect("target machine");
    let target_data = machine.get_target_data();
    let context = Context::create();
    let outer_ty = basic_ty(
        &context,
        &module.structs,
        &module.enums,
        &LirType::Struct(outer),
    )
    .expect("outer LLVM type");
    assert_eq!(target_data.get_abi_size(&outer_ty), 32);
    assert_eq!(target_data.get_abi_alignment(&outer_ty), 16);
    let wrapped_ty = basic_ty(
        &context,
        &module.structs,
        &module.enums,
        &LirType::Enum(wrapped),
    )
    .expect("wrapped LLVM type");
    assert_eq!(target_data.get_abi_size(&wrapped_ty), 48);
    assert_eq!(target_data.get_abi_alignment(&wrapped_ty), 16);

    let assertions = c_layout_assertions(&module).expect("C assertions");
    assert!(
        assertions.find("scoop_c_layout_0").unwrap() < assertions.find("scoop_c_layout_1").unwrap(),
        "nested declaration must precede its user:\n{assertions}"
    );
    assert!(assertions.contains("offsetof(scoop_c_layout_1, _field_1) == 2"));
    assert!(assertions.contains("_Alignof(scoop_c_layout_1) == 16"));
    let source = std::env::temp_dir().join(format!(
        "scoop_c_layout_assertions_{}.c",
        std::process::id()
    ));
    std::fs::write(&source, &assertions).expect("write generated C");
    let status = std::process::Command::new("cc")
        .args(["-std=c11", "-fsyntax-only"])
        .arg(&source)
        .status()
        .expect("run C compiler");
    std::fs::remove_file(&source).ok();
    assert!(status.success(), "generated C assertions must compile");

    module.extern_functions.alloc(ExternFunction {
        source_name: "swap".to_string(),
        native_symbol: "native_swap".to_string(),
        library: "fixture".to_string(),
        calling_convention: scoop_lir::CallingConvention::Cdecl,
        params: vec![LirType::Struct(outer)],
        return_type: LirType::Struct(outer),
        kind: ExternFunctionKind::C {
            bridge_symbol: "scoop_c_bridge_0".to_string(),
            params: vec![scoop_lir::CType::Struct(outer)],
            return_type: scoop_lir::CType::Struct(outer),
        },
    });
    module.callback_bridges.alloc(scoop_lir::CallbackBridge {
        source_name: "swapCallback".to_string(),
        bridge_symbol: "scoop_callback_bridge_0".to_string(),
        trampoline_symbol: "scoop_c_callback_0".to_string(),
        params: vec![scoop_lir::CType::Struct(outer)],
        return_type: scoop_lir::CType::Struct(outer),
    });
    for (adapter, mode) in [
        (
            "scoop_foreign_callback_adapter_0",
            scoop_lir::ForeignCallbackMode::Reusable,
        ),
        (
            "scoop_foreign_callback_adapter_1",
            scoop_lir::ForeignCallbackMode::OneShot,
        ),
    ] {
        module
            .foreign_callback_bridges
            .alloc(scoop_lir::ForeignCallbackBridge {
                adapter_symbol: adapter.to_string(),
                trampoline_symbol: "scoop_foreign_callback_0".to_string(),
                signature_symbol: "scoop_foreign_callback_signature_0".to_string(),
                params: vec![scoop_lir::CType::Int, scoop_lir::CType::Pointer],
                return_type: scoop_lir::CType::Int,
                context_index: 1,
                mode,
            });
    }
    let bridge = c_bridge_source(&module)
        .expect("C bridge")
        .expect("C extern needs a bridge");
    assert!(bridge.contains("extern scoop_c_layout_1 native_swap(scoop_c_layout_1);"));
    assert!(bridge.contains("void scoop_c_bridge_0(void *result, const void *arg0)"));
    assert!(bridge.contains("memcpy(result, &native_result, sizeof(native_result));"));
    assert!(
        bridge.contains("extern void scoop_callback_bridge_0(void *result, const void *arg0);")
    );
    assert!(bridge.contains("scoop_c_layout_1 scoop_c_callback_0(scoop_c_layout_1 arg0)"));
    assert!(bridge.contains("scoop_callback_bridge_0(&result, &arg0);"));
    assert_eq!(
        bridge
            .matches("const unsigned char scoop_foreign_callback_signature_0 = 0;")
            .count(),
        1,
        "one signature/context shape must emit one descriptor:\n{bridge}"
    );
    assert_eq!(
        bridge
            .matches("int64_t scoop_foreign_callback_0(int64_t arg0, void * arg1)")
            .count(),
        1,
        "registrations sharing a signature/context shape must share one trampoline:\n{bridge}"
    );
    assert!(bridge.contains("int64_t result = {0};"));
    assert!(bridge.contains("const void *arguments[1] = {&arg0};"));
    assert!(bridge.contains(
            "scoop_runtime_callback_invoke(arg1, &scoop_foreign_callback_signature_0, &result, arguments)"
        ));
    let bridge_source = std::env::temp_dir().join(format!(
        "scoop_c_foreign_callback_bridge_{}.c",
        std::process::id()
    ));
    std::fs::write(&bridge_source, &bridge).expect("write generated callback C");
    let status = std::process::Command::new("cc")
        .args(["-std=c11", "-fsyntax-only"])
        .arg(&bridge_source)
        .status()
        .expect("run C compiler");
    std::fs::remove_file(&bridge_source).ok();
    assert!(status.success(), "generated callback C must compile");

    let ir = ir_of(&module);
    assert!(
        ir.contains("getelementptr i8, ptr %managed_object, i32 32"),
        "over-aligned array data must start at offset 32:\n{ir}"
    );
    assert!(
        ir.contains(
            "@scoop_runtime_finish_tlab_alloc(ptr %tlab_object, ptr @scoop_td_ArrayOuter, i64 64)"
        ) && ir.contains("@scoop_runtime_alloc_slow(ptr @scoop_td_ArrayOuter, i64 64)"),
        "one 32-byte element plus the aligned 32-byte header must flow through the 64-byte TLAB check:\n{ir}"
    );
}
