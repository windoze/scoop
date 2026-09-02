use inkwell::OptimizationLevel;
use inkwell::targets::FileType;
use la_arena::Arena;
use scoop_lir::{
    BasicBlock, CallDestination, CallSite, CallTargets, DirectCallSignature, DispatchKind,
    DispatchSlot, EnumDef, EnumFieldRepr, EnumRepr, EnumVariantRepr, GcEffect, Global, GlobalInit,
    IndirectResultCallSignature, ItableRecord, Layout, LayoutKind, LirMeta, Local, MANAGED_PTR,
    METADATA_PTR, NativeBorrowedResultRoot, PointerKind, RAW_PTR, ResultStorage, Temp,
    TypeDescriptor, TypeDescriptorRef, TypeDescriptorScan, TypedCall, VoidCallSignature,
    WellKnownLayouts, WellKnownTypeDescriptors,
};

use super::*;

#[path = "runtime_collector_tests.rs"]
mod runtime_collector_tests;

fn host_profile() -> TargetProfile {
    TargetProfile::resolve_host().expect("supported host target")
}

fn host_managed_address_space() -> ManagedAddressSpace {
    host_profile().managed_address_space_contract()
}

enum TestCallProtocol {
    Managed(u64),
    NoGc,
    NativeSafe {
        safepoint: u64,
        destination: scoop_lir::NativeSafeCallDestination,
    },
    NativeBorrowed {
        safepoint: u64,
        destination: scoop_lir::NativeBorrowedCallDestination,
        result: NativeBorrowedResultRoot,
    },
}

enum TestTypedCall {
    Void {
        signature: scoop_lir::VoidCallSignatureId,
        args: Vec<Value>,
    },
    Direct {
        signature: scoop_lir::DirectCallSignatureId,
        out: TempId,
        args: Vec<Value>,
    },
    IndirectResult {
        signature: scoop_lir::IndirectResultCallSignatureId,
        storage: scoop_lir::LocalId,
        args: Vec<Value>,
    },
}

fn bind_test_call<Destination: Copy>(
    targets: &mut scoop_lir::ProtocolCallTargets<Destination>,
    destination: Destination,
    call: TestTypedCall,
) -> scoop_lir::TypedCall<Destination> {
    match call {
        TestTypedCall::Void { signature, args } => {
            let target = targets.void.alloc(scoop_lir::CallTarget {
                destination,
                signature,
            });
            scoop_lir::TypedCall::Void { target, args }
        }
        TestTypedCall::Direct {
            signature,
            out,
            args,
        } => {
            let target = targets.direct.alloc(scoop_lir::CallTarget {
                destination,
                signature,
            });
            scoop_lir::TypedCall::Direct { target, out, args }
        }
        TestTypedCall::IndirectResult {
            signature,
            storage,
            args,
        } => {
            let target = targets.indirect_result.alloc(scoop_lir::CallTarget {
                destination,
                signature,
            });
            scoop_lir::TypedCall::IndirectResult {
                target,
                storage,
                args,
            }
        }
    }
}

fn test_safepoint(raw: u64) -> scoop_lir::SafepointId {
    scoop_lir::SafepointId::new(raw).expect("test safepoint ids are non-zero")
}

fn statepoint_value(
    source: scoop_lir::CallerRootSource,
    ty: LirType,
    offsets: &[u64],
) -> scoop_lir::StatepointLiveValue {
    scoop_lir::StatepointLiveValue {
        source,
        ty,
        leaves: scoop_lir::ManagedLeafPaths::new(
            offsets
                .iter()
                .copied()
                .map(|byte_offset| scoop_lir::ManagedLeafPath { byte_offset })
                .collect(),
        )
        .expect("test roots have at least one sorted managed leaf"),
    }
}

fn statepoint_live(values: Vec<scoop_lir::StatepointLiveValue>) -> scoop_lir::StatepointLiveSet {
    scoop_lir::StatepointLiveSet::new(values).expect("test roots are source ordered and unique")
}

fn set_managed_live(site: &mut CallSite, live: scoop_lir::StatepointLiveSet) {
    let CallSite::Managed(site) = site else {
        panic!("test root plan requires a managed call site")
    };
    site.live = live;
}

fn protocol_site(
    targets: &mut CallTargets,
    destination: CallDestination,
    protocol: TestCallProtocol,
    call: TestTypedCall,
) -> CallSite {
    match protocol {
        TestCallProtocol::Managed(safepoint) => {
            let destination = scoop_lir::ManagedCallDestination::from_view(destination)
                .expect("managed test destination");
            CallSite::Managed(scoop_lir::ManagedCallSite {
                call: bind_test_call(&mut targets.managed_targets, destination, call),
                safepoint: test_safepoint(safepoint),
                live: scoop_lir::StatepointLiveSet::default(),
            })
        }
        TestCallProtocol::NoGc => {
            let destination = scoop_lir::NoGcCallDestination::from_view(destination)
                .expect("NoGC test destination");
            CallSite::NoGc(scoop_lir::NoGcCallSite {
                call: bind_test_call(&mut targets.no_gc_targets, destination, call),
            })
        }
        TestCallProtocol::NativeSafe {
            safepoint,
            destination: typed_destination,
        } => {
            assert_eq!(destination, typed_destination.view());
            CallSite::NativeSafe(scoop_lir::NativeSafeCallSite {
                call: bind_test_call(&mut targets.native_safe_targets, typed_destination, call),
                safepoint: test_safepoint(safepoint),
                roots: scoop_lir::NativeSafeRootSet::default(),
            })
        }
        TestCallProtocol::NativeBorrowed {
            safepoint,
            destination: typed_destination,
            result,
        } => {
            assert_eq!(destination, typed_destination.view());
            CallSite::NativeBorrowed(scoop_lir::NativeBorrowedCallSite {
                call: bind_test_call(
                    &mut targets.native_borrowed_targets,
                    typed_destination,
                    call,
                ),
                safepoint: test_safepoint(safepoint),
                roots: scoop_lir::NativeBorrowedRootSet::new(Vec::new(), result),
            })
        }
    }
}

fn void_site(
    targets: &mut CallTargets,
    destination: CallDestination,
    protocol: TestCallProtocol,
    params: Vec<LirType>,
    args: Vec<Value>,
) -> CallSite {
    let signature = targets.void_signatures.alloc(VoidCallSignature {
        params,
        calling_convention: scoop_lir::CallingConvention::Cdecl,
    });
    protocol_site(
        targets,
        destination,
        protocol,
        TestTypedCall::Void { signature, args },
    )
}

fn direct_site(
    targets: &mut CallTargets,
    destination: CallDestination,
    protocol: TestCallProtocol,
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
    protocol_site(
        targets,
        destination,
        protocol,
        TestTypedCall::Direct {
            signature,
            out,
            args,
        },
    )
}

fn indirect_result_site(
    targets: &mut CallTargets,
    destination: CallDestination,
    protocol: TestCallProtocol,
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
    protocol_site(
        targets,
        destination,
        protocol,
        TestTypedCall::IndirectResult {
            signature,
            storage,
            args,
        },
    )
}

fn managed_invoke(
    site: CallSite,
    normal: scoop_lir::BlockId,
    unwind: scoop_lir::BlockId,
) -> scoop_lir::InvokeSite {
    let CallSite::Managed(site) = site else {
        panic!("test invoke helper requires a managed call site")
    };
    scoop_lir::InvokeSite::Managed(scoop_lir::ManagedInvokeSite {
        call: site.call,
        safepoint: site.safepoint,
        roots: scoop_lir::ExceptionalRootSet::default(),
        normal,
        unwind,
    })
}

fn dispatch_destination(targets: &mut CallTargets, table: Value, index: u32) -> CallDestination {
    let slot = targets.dispatch_slots.alloc(DispatchSlot {
        kind: DispatchKind::Virtual,
        index,
    });
    CallDestination::Dispatch { table, slot }
}

fn managed_runtime(function: scoop_lir::ManagedRuntimeFunction) -> CallDestination {
    CallDestination::Runtime(scoop_lir::RuntimeFunction::Managed(function))
}

fn no_gc_runtime(function: scoop_lir::NoGcRuntimeFunction) -> CallDestination {
    CallDestination::Runtime(scoop_lir::RuntimeFunction::NoGc(function))
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
        managed_runtime(scoop_lir::ManagedRuntimeFunction::StringConcat),
        TestCallProtocol::Managed(1),
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
fn ir_of(module: &Module) -> String {
    let machine = host_target_machine().expect("target machine");
    let context = Context::create();
    let llvm = emit_llvm_module(&context, module, &machine, host_profile()).expect("emit module");
    llvm.verify().expect("valid LLVM module");
    llvm.print_to_string().to_string()
}

fn rewritten_ir_of(module: &Module) -> String {
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

mod arrays;
mod c_layout;
mod closures;
mod enums;
mod exceptions;
mod moving_gc;
mod objects;
mod platform;
mod smoke;
mod statepoints;

use enums::enum_module;
use exceptions::exceptions_module;
use objects::heap_module;
