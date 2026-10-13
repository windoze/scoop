use super::*;

mod arguments;
mod invoke;

pub(crate) enum RootArguments<'a> {
    Unused,
    Build(
        scoop_lir::ManagedCallDestination,
        &'a scoop_lir::ScoopAbiSignature,
    ),
}

pub(crate) struct RootMain<'a> {
    pub target: scoop_lir::LocalFunctionRef,
    pub signature: &'a scoop_lir::ScoopAbiSignature,
    pub arguments: RootArguments<'a>,
}

pub(super) fn root_gateway(
    producer: scoop_identity::ConeIdentity,
    main_body: scoop_identity::MainCallableBodyId,
    main: RootMain<'_>,
    failure_root: scoop_lir::GlobalId,
    signatures: &crate::abi::plans::Signatures,
) -> LoweredFunction {
    let callable_body = scoop_lir::CallableBodyIdentity::for_root_gateway(producer, main_body)
        .expect("a validated executable entry derives one root-gateway identity");
    let mut call_targets = scoop_lir::CallTargets::default();
    let mut pending_safepoints = PendingSafepointSites::default();

    let mut blocks = Arena::new();
    let entry = blocks.alloc(scoop_lir::BasicBlock {
        name: "entry".to_string(),
        instructions: Vec::new(),
        terminator: scoop_lir::Terminator::Unreachable,
    });
    let call_main = if matches!(main.arguments, RootArguments::Unused) {
        entry
    } else {
        blocks.alloc(scoop_lir::BasicBlock {
            name: "main".to_string(),
            instructions: Vec::new(),
            terminator: scoop_lir::Terminator::Unreachable,
        })
    };
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
    let mut temps = Arena::new();
    let args = arguments::build_arguments(
        &main,
        &mut blocks,
        &mut call_targets,
        &mut temps,
        &mut pending_safepoints,
        (entry, call_main, unwind),
    );
    let (invoke, exit_code) = invoke::invoke_main(
        main.target,
        main.signature,
        args,
        &mut call_targets,
        &mut temps,
        &mut pending_safepoints,
        (success, unwind),
    );
    blocks[call_main]
        .instructions
        .push(scoop_lir::Instruction::Invoke { site: invoke });
    blocks[call_main].terminator = scoop_lir::Terminator::Br(success);

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
    let runtime = signatures.runtime(scoop_lir::RuntimeFunction::Managed(
        scoop_lir::ManagedRuntimeFunction::MaterializeException,
    ));
    let scoop_lir::AbiReturn::Direct(reference) = runtime.result() else {
        unreachable!("exception materialization returns a managed reference")
    };
    let signature = call_targets
        .direct_signatures
        .alloc(scoop_lir::DirectCallSignature::new(
            runtime.arguments().to_vec(),
            reference.clone(),
            runtime.calling_convention(),
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

    let gateway_signature = signatures.plans.scoop(signatures.requests.root);
    let scoop_lir::AbiReturn::Direct(result) = gateway_signature.result() else {
        unreachable!("root gateway returns an exit status")
    };
    blocks[success]
        .instructions
        .push(scoop_lir::Instruction::RawStore {
            pointer: scoop_lir::Value::Param(2),
            value: exit_code,
            pointee: result.value().clone(),
        });
    LoweredFunction {
        function: scoop_lir::Function {
            callable_body,
            gc_effect: scoop_lir::GcEffect::Managed,
            signature: gateway_signature.clone(),
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
