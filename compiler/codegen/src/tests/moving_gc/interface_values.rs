use super::*;
use inkwell::passes::PassBuilderOptions;
use inkwell::values::AnyValue;
use scoop_lir::{LocalId, OptimizationMode, TargetProfileId};

const TARGETS: [TargetProfileId; 3] = [
    scoop_target_data::DARWIN_AARCH64_ID,
    scoop_target_data::LINUX_X86_64_GNU_ID,
    scoop_target_data::LINUX_X86_64_MUSL_ID,
];

fn fixture() -> (Module, [LocalId; 4], TempId) {
    let mut module = values_module();
    module.output = scoop_lir::LirOutput::Library;
    module.functions.truncate(1);
    let function = &mut module.functions[0];
    function.gc_effect = GcEffect::Managed;
    function.signature = plain_scoop_signature(
        vec![MANAGED_PTR, METADATA_PTR, MANAGED_PTR, METADATA_PTR],
        LirType::Interface,
    );
    function.call_targets = CallTargets::default();
    function.locals = Arena::default();
    function.temps = Arena::default();
    function.blocks = Arena::default();
    let locals = std::array::from_fn(|index| {
        function.locals.alloc(Local::new(
            format!("copy{index}"),
            scoop_lir::LocalStorage::NonZero(abi_value_with_layout(
                LirType::Interface,
                16,
                8,
                RefScan::References(vec![0]),
            )),
        ))
    });
    let original = function.temps.alloc(Temp {
        ty: LirType::Interface,
    });
    let mut instructions = vec![Instruction::MakeAggregate {
        out: original,
        elements: vec![Value::Param(0), Value::Param(1)],
    }];
    let mut value = Value::Temp(original);
    for local in locals {
        instructions.push(Instruction::Store { local, value });
        value = Value::Local(local);
    }
    function.entry = function.blocks.alloc(BasicBlock {
        name: "copies".into(),
        instructions,
        terminator: Terminator::Return { value: Some(value) },
    });
    refresh_module_safepoints(&mut module);
    (module, locals, original)
}

fn check(
    mut module: Module,
    target: TargetProfileId,
    mode: OptimizationMode,
    reloads: usize,
    returned_parameter: Option<u32>,
) {
    module.meta = string_metadata_for(scoop_target_data::layout(target).unwrap());
    refresh_module_safepoints(&mut module);
    let profile =
        ValidatedBackendProfile::for_selection(scoop_target_data::selection(target).unwrap())
            .unwrap()
            .with_optimization(mode);
    let machine = profile.create_target_machine().unwrap();
    let context = Context::create();
    let llvm = emit_llvm_module(&context, &module, &machine, profile).unwrap();
    let function = llvm.get_function(module.functions[0].symbol()).unwrap();
    let raw = function.print_to_string().to_string();
    assert_eq!(
        raw.matches("load volatile ptr addrspace(1)").count(),
        reloads,
        "{raw}"
    );
    let expected = statepoint::expectations(&module).unwrap();
    let plan = statepoint::rewrite(&llvm, &machine, &expected, profile).unwrap();
    for site in module.functions[0].safepoints.iter() {
        assert_eq!(plan.root_count(site.runtime_id().get()), Some(1));
    }
    assert_eq!(plan.site_count(), module.functions[0].safepoints.len());
    llvm.run_passes(
        "function(verify<safepoint-ir>),verify",
        &machine,
        PassBuilderOptions::create(),
    )
    .unwrap();
    if let Some(parameter) = returned_parameter {
        let result = function
            .get_last_basic_block()
            .unwrap()
            .get_terminator()
            .unwrap();
        assert_eq!(
            result.get_operand(0).unwrap().value(),
            function.get_nth_param(parameter)
        );
    }
    let directory = tempfile::tempdir().unwrap();
    machine
        .write_to_file(
            &llvm,
            FileType::Object,
            &directory.path().join("interfaces.o"),
        )
        .unwrap();
}

#[test]
fn straight_interface_copy_chains_reuse_fresh_snapshots_only_in_x86_release() {
    for target in TARGETS {
        for mode in [OptimizationMode::Debug, OptimizationMode::Release] {
            check(
                fixture().0,
                target,
                mode,
                if mode == OptimizationMode::Release
                    && target != scoop_target_data::DARWIN_AARCH64_ID
                {
                    0
                } else {
                    4
                },
                None,
            );
        }
    }
}

#[test]
fn overwriting_the_source_keeps_both_parts_of_earlier_interface_copies() {
    for target in TARGETS {
        for (index, ty) in [MANAGED_PTR, METADATA_PTR].into_iter().enumerate() {
            let (mut module, locals, _) = fixture();
            let function = &mut module.functions[0];
            let replacement = function.temps.alloc(Temp {
                ty: LirType::Interface,
            });
            let result = function.temps.alloc(Temp { ty: ty.clone() });
            let block = &mut function.blocks[function.entry];
            block.instructions.extend([
                Instruction::MakeAggregate {
                    out: replacement,
                    elements: vec![Value::Param(2), Value::Param(3)],
                },
                Instruction::Store {
                    local: locals[0],
                    value: Value::Temp(replacement),
                },
                Instruction::ExtractValue {
                    out: result,
                    aggregate: Value::Local(locals[3]),
                    index: index as u32,
                },
            ]);
            block.terminator = Terminator::Return {
                value: Some(Value::Temp(result)),
            };
            function.signature = plain_scoop_signature(
                vec![MANAGED_PTR, METADATA_PTR, MANAGED_PTR, METADATA_PTR],
                ty,
            );
            check(
                module,
                target,
                OptimizationMode::Release,
                if target == scoop_target_data::DARWIN_AARCH64_ID {
                    4
                } else {
                    0
                },
                (target != scoop_target_data::DARWIN_AARCH64_ID).then_some(index as u32),
            );
        }
    }
}

#[derive(Clone, Copy)]
enum Boundary {
    NoGc,
    Managed,
    Poll,
    Address,
    RawWrite,
    Atomic,
    Block,
}

fn boundary(kind: Boundary) -> Module {
    let (mut module, locals, _) = fixture();
    let function = &mut module.functions[0];
    let mut instructions = Vec::new();
    match kind {
        Boundary::NoGc => {
            let out = function.temps.alloc(Temp { ty: LirType::I64 });
            let site = direct_site(
                &mut function.call_targets,
                TestCallProtocol::NoGc {
                    destination: no_gc_runtime(scoop_lir::NoGcRuntimeFunction::GcStats),
                },
                vec![],
                (LirType::I64, RefScan::None),
                out,
                vec![],
            );
            instructions.push(Instruction::Call { site });
        }
        Boundary::Managed | Boundary::Poll => {
            let mut site = void_site(
                &mut function.call_targets,
                TestCallProtocol::Managed {
                    safepoint: 1,
                    destination: managed_runtime(scoop_lir::ManagedRuntimeFunction::Safepoint),
                },
                vec![],
                vec![],
            );
            set_managed_live(
                &mut site,
                statepoint_live(vec![statepoint_value(
                    scoop_lir::CallerRootSource::Local(locals[3]),
                    LirType::Interface,
                    &[0],
                )]),
            );
            if matches!(kind, Boundary::Poll) {
                let CallSite::Managed(site) = site else {
                    unreachable!()
                };
                let TypedCall::Void { target, .. } = site.call else {
                    unreachable!()
                };
                instructions.push(Instruction::ManagedPoll {
                    site: scoop_lir::ManagedPollSite {
                        kind: scoop_lir::ManagedPollKind::Full,
                        target,
                        safepoint: site.safepoint,
                        live: site.live,
                    },
                });
            } else {
                instructions.push(Instruction::Call { site });
            }
        }
        Boundary::Address | Boundary::RawWrite => {
            let pointer = function.temps.alloc(Temp { ty: RAW_PTR });
            instructions.push(Instruction::LocalAddress {
                out: pointer,
                local: locals[3],
            });
            if matches!(kind, Boundary::RawWrite) {
                instructions.extend([
                    Instruction::Store {
                        local: locals[3],
                        value: Value::Local(locals[0]),
                    },
                    Instruction::RawStore {
                        pointer: Value::Temp(pointer),
                        value: Value::Param(2),
                        pointee: abi_value_with_layout(
                            MANAGED_PTR,
                            8,
                            8,
                            RefScan::References(vec![0]),
                        ),
                    },
                ]);
            }
        }
        Boundary::Atomic => instructions.push(Instruction::AtomicStore {
            location: scoop_lir::AtomicLocation {
                object: Value::Param(0),
                offset: 16,
                kind: scoop_lir::AtomicValueKind::Reference,
            },
            value: Value::Param(2),
            order: scoop_lir::AtomicStoreOrder::Release,
        }),
        Boundary::Block => {
            let next = function.blocks.alloc(BasicBlock {
                name: "next".into(),
                instructions: vec![],
                terminator: Terminator::Return {
                    value: Some(Value::Local(locals[3])),
                },
            });
            function.blocks[function.entry].terminator = Terminator::Br(next);
        }
    }
    function.blocks[function.entry]
        .instructions
        .extend(instructions);
    module
}

#[test]
fn calls_polls_alias_writes_and_control_flow_end_interface_snapshot_reuse() {
    for target in TARGETS {
        for kind in [
            Boundary::NoGc,
            Boundary::Managed,
            Boundary::Poll,
            Boundary::Address,
            Boundary::RawWrite,
            Boundary::Atomic,
            Boundary::Block,
        ] {
            check(
                boundary(kind),
                target,
                OptimizationMode::Release,
                1 + usize::from(matches!(kind, Boundary::RawWrite))
                    + if target == scoop_target_data::DARWIN_AARCH64_ID {
                        3
                    } else {
                        0
                    },
                None,
            );
        }
    }
}

#[test]
fn canonical_interface_temporaries_reload_after_a_collecting_call() {
    for target in TARGETS {
        let (mut module, _, original) = fixture();
        let function = &mut module.functions[0];
        let mut site = void_site(
            &mut function.call_targets,
            TestCallProtocol::Managed {
                safepoint: 1,
                destination: managed_runtime(scoop_lir::ManagedRuntimeFunction::Safepoint),
            },
            vec![],
            vec![],
        );
        set_managed_live(
            &mut site,
            statepoint_live(vec![statepoint_value(
                scoop_lir::CallerRootSource::Temp(original),
                LirType::Interface,
                &[0],
            )]),
        );
        let block = &mut function.blocks[function.entry];
        block.instructions.push(Instruction::Call { site });
        block.terminator = Terminator::Return {
            value: Some(Value::Temp(original)),
        };
        check(
            module,
            target,
            OptimizationMode::Release,
            if target == scoop_target_data::DARWIN_AARCH64_ID {
                5
            } else {
                1
            },
            None,
        );
    }
}
