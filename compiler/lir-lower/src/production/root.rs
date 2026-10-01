use super::*;

pub(super) fn root_gateway(
    producer: scoop_identity::ConeIdentity,
    main_body: scoop_identity::MainCallableBodyId,
    main: scoop_lir::LocalFunctionRef,
    failure_root: scoop_lir::GlobalId,
) -> LoweredFunction {
    let callable_body = scoop_lir::CallableBodyIdentity::for_root_gateway(producer, main_body)
        .expect("a validated executable entry derives one root-gateway identity");
    let mut call_targets = scoop_lir::CallTargets::default();
    let signature = call_targets
        .void_signatures
        .alloc(scoop_lir::VoidCallSignature::new(
            Vec::new(),
            scoop_lir::CallingConvention::Cdecl,
        ));
    let mut pending_safepoints = PendingSafepointSites::default();

    let mut blocks = Arena::new();
    let entry = blocks.alloc(scoop_lir::BasicBlock {
        name: "entry".to_string(),
        instructions: Vec::new(),
        terminator: scoop_lir::Terminator::Unreachable,
    });
    let success = blocks.alloc(scoop_lir::BasicBlock {
        name: "success".to_string(),
        instructions: Vec::new(),
        terminator: scoop_lir::Terminator::Return {
            value: Some(scoop_lir::Value::IntegerConst(
                scoop_lir::LirIntegerConstant::Unsigned32(0),
            )),
        },
    });
    let unwind = blocks.alloc(scoop_lir::BasicBlock {
        name: "failure".to_string(),
        instructions: Vec::new(),
        terminator: scoop_lir::Terminator::Return {
            value: Some(scoop_lir::Value::IntegerConst(
                scoop_lir::LirIntegerConstant::Unsigned32(1),
            )),
        },
    });
    let invoke = match main {
        scoop_lir::LocalFunctionRef::Managed(main) => {
            let target = call_targets
                .managed_targets
                .void
                .alloc(scoop_lir::CallTarget {
                    destination: scoop_lir::ManagedCallDestination::local(main),
                    signature,
                });
            scoop_lir::InvokeSite::Managed(scoop_lir::ManagedInvokeSite {
                call: scoop_lir::ManagedTypedCall::Void {
                    target,
                    args: Vec::new(),
                },
                safepoint: pending_safepoints.allocate(scoop_lir::SafepointSiteRole::ManagedInvoke),
                roots: scoop_lir::ExceptionalRootSet::default(),
                normal: success,
                unwind,
            })
        }
        scoop_lir::LocalFunctionRef::NoGc(main) => {
            let target = call_targets
                .no_gc_targets
                .void
                .alloc(scoop_lir::CallTarget {
                    destination: scoop_lir::NoGcCallDestination::local(main),
                    signature,
                });
            scoop_lir::InvokeSite::NoGc(scoop_lir::NoGcInvokeSite {
                call: scoop_lir::NoGcTypedCall::Void {
                    target,
                    args: Vec::new(),
                },
                normal: success,
                unwind,
            })
        }
    };
    blocks[entry]
        .instructions
        .push(scoop_lir::Instruction::Invoke { site: invoke });
    blocks[entry].terminator = scoop_lir::Terminator::Br(success);

    let mut temps = Arena::new();
    let exception_record = temps.alloc(scoop_lir::Temp {
        ty: scoop_lir::LirType::ExceptionRecord,
    });
    let raw_exception = temps.alloc(scoop_lir::Temp {
        ty: scoop_lir::RAW_PTR,
    });
    let managed_exception = temps.alloc(scoop_lir::Temp {
        ty: scoop_lir::MANAGED_PTR,
    });
    let payload = managed_exception;
    let managed_exception = temps.alloc(scoop_lir::Temp {
        ty: scoop_lir::MANAGED_PTR,
    });
    let reference = scoop_lir::AbiValue::new(
        scoop_lir::MANAGED_PTR,
        scoop_lir::AbiNonZeroLayout::new(8, 8)
            .expect("the exception reference has the target pointer layout"),
        scoop_lir::RefScan::References(vec![0]),
    )
    .expect("the exception reference has one managed leaf");
    let signature = call_targets
        .direct_signatures
        .alloc(scoop_lir::DirectCallSignature::new(
            vec![scoop_lir::AbiArgument::Direct(reference.clone())],
            reference,
            scoop_lir::CallingConvention::Cdecl,
        ));
    let target = call_targets
        .managed_targets
        .direct
        .alloc(scoop_lir::CallTarget {
            destination: scoop_lir::ManagedCallDestination::runtime(
                scoop_lir::ManagedRuntimeFunction::MaterializeException,
            ),
            signature,
        });
    blocks[unwind].instructions.extend([
        scoop_lir::Instruction::LandingPad {
            record: exception_record,
            raw: raw_exception,
        },
        scoop_lir::Instruction::BeginCatch {
            out: payload,
            raw: scoop_lir::Value::Temp(raw_exception),
        },
        scoop_lir::Instruction::Call {
            site: scoop_lir::CallSite::Managed(scoop_lir::ManagedCallSite {
                call: scoop_lir::ManagedTypedCall::Direct {
                    target,
                    out: managed_exception,
                    args: vec![scoop_lir::AbiCallArgument::Direct(scoop_lir::Value::Temp(
                        payload,
                    ))],
                },
                safepoint: pending_safepoints.allocate(scoop_lir::SafepointSiteRole::ManagedCall),
                live: scoop_lir::StatepointLiveSet::default(),
            }),
        },
        scoop_lir::Instruction::GlobalStore {
            global: failure_root,
            value: scoop_lir::Value::Temp(managed_exception),
        },
        scoop_lir::Instruction::EndCatch,
    ]);

    let result = scoop_lir::AbiValue::new(
        scoop_lir::LirType::I32,
        scoop_lir::AbiNonZeroLayout::new(4, 4)
            .expect("the root gateway result has a valid uint32 layout"),
        scoop_lir::RefScan::None,
    )
    .expect("the root gateway result layout matches uint32 storage");
    LoweredFunction {
        function: scoop_lir::Function {
            callable_body,
            gc_effect: scoop_lir::GcEffect::Managed,
            signature: scoop_lir::ScoopAbiSignature::new(
                Vec::new(),
                scoop_lir::AbiReturn::Direct(result),
                scoop_lir::CallingConvention::Cdecl,
            ),
            call_targets,
            safepoints: scoop_lir::SafepointIdentities::default(),
            locals: Arena::new(),
            temps,
            blocks,
            entry,
        },
        loop_header_polls: Vec::new(),
        pending_safepoints,
    }
}
