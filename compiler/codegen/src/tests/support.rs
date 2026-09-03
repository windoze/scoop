use super::*;

mod calls;

pub(super) use calls::*;

pub(super) fn host_profile() -> TargetProfile {
    TargetProfile::resolve_host().expect("supported host target")
}

pub(super) fn host_managed_address_space() -> ManagedAddressSpace {
    host_profile().managed_address_space_contract()
}

pub(super) fn string_metadata() -> LirMeta {
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

pub(super) fn array_type(
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
pub(super) fn values_module() -> Module {
    let mut globals = Arena::default();
    let hello = globals.alloc(Global {
        symbol: "scoop.string.0".to_string(),
        address_kind: PointerKind::Managed,
        scan: RefScan::None,
        init: GlobalInit::StringConst("hello, ".to_string()),
    });
    let world = globals.alloc(Global {
        symbol: "scoop.string.1".to_string(),
        address_kind: PointerKind::Managed,
        scan: RefScan::None,
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
        TestCallProtocol::Managed {
            safepoint: 1,
            destination: managed_runtime(scoop_lir::ManagedRuntimeFunction::StringConcat),
        },
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
        extern_functions: Default::default(),
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

/// The LLVM IR text of a module, verified, before the statepoint rewrite.
pub(super) fn ir_of(module: &Module) -> String {
    let machine = host_target_machine().expect("target machine");
    let context = Context::create();
    let llvm = emit_llvm_module(&context, module, &machine, host_profile()).expect("emit module");
    llvm.verify().expect("valid LLVM module");
    llvm.print_to_string().to_string()
}

pub(super) fn rewritten_ir_of(module: &Module) -> String {
    let machine = host_target_machine().expect("target machine");
    let context = Context::create();
    let llvm = emit_llvm_module(&context, module, &machine, host_profile()).expect("emit module");
    llvm.verify().expect("valid pre-statepoint module");
    let expected = statepoint::expectations(module).expect("complete safepoint manifest");
    statepoint::rewrite(&llvm, &machine).expect("rewrite statepoints");
    llvm.verify().expect("valid relocated module");
    statepoint::verify_rewritten(&llvm, &expected, host_profile())
        .expect("rewritten manifest agrees with LIR");
    llvm.print_to_string().to_string()
}
