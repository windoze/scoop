use super::*;

pub(super) fn gateway(
    body: CallableBodyIdentity,
    target: LocalFunctionRef,
    failure_root: Option<GlobalId>,
    task_descriptor: TypeDescriptorRef,
) -> Function {
    let mut function = support::function(body, GcEffect::Managed);
    let result = AbiValue::new(
        LirType::I32,
        AbiNonZeroLayout::new(4, 4).unwrap(),
        RefScan::None,
    )
    .unwrap();
    function.signature =
        ScoopAbiSignature::new(vec![], AbiReturn::Direct(result), CallingConvention::Cdecl);
    let entry = function.entry;
    let success = function.blocks.alloc(exit(0));
    let failure = function.blocks.alloc(exit(1));
    let targets = &mut function.call_targets;
    let signature = targets
        .void_signatures
        .alloc(VoidCallSignature::new(vec![], CallingConvention::Cdecl));
    let poll = targets.managed_targets.void.alloc(CallTarget {
        destination: ManagedCallDestination::Runtime(ManagedRuntimeFunction::Safepoint),
        signature,
    });
    let mut identities = vec![(
        SafepointSiteRef::from_u32(0),
        SafepointIdentity::new(
            function.callable_body.id(),
            SafepointSiteRole::ManagedPoll,
            0,
        )
        .unwrap(),
    )];
    let invoke = match target {
        LocalFunctionRef::Managed(target) => {
            let target = targets.managed_targets.void.alloc(CallTarget {
                destination: ManagedCallDestination::Local(target),
                signature,
            });
            identities.push((
                SafepointSiteRef::from_u32(1),
                SafepointIdentity::new(
                    function.callable_body.id(),
                    SafepointSiteRole::ManagedInvoke,
                    0,
                )
                .unwrap(),
            ));
            InvokeSite::Managed(ManagedInvokeSite {
                call: ManagedTypedCall::Void {
                    target,
                    args: vec![],
                },
                safepoint: SafepointSiteRef::from_u32(1),
                roots: ExceptionalRootSet::default(),
                normal: success,
                unwind: failure,
            })
        }
        LocalFunctionRef::NoGc(target) => {
            let target = targets.no_gc_targets.void.alloc(CallTarget {
                destination: NoGcCallDestination::Local(target),
                signature,
            });
            InvokeSite::NoGc(NoGcInvokeSite {
                call: NoGcTypedCall::Void {
                    target,
                    args: vec![],
                },
                normal: success,
                unwind: failure,
            })
        }
    };
    function.blocks[entry].instructions = vec![
        Instruction::ManagedPoll {
            site: ManagedPollSite {
                target: poll,
                safepoint: SafepointSiteRef::from_u32(0),
                live: StatepointLiveSet::default(),
            },
        },
        Instruction::Invoke { site: invoke },
    ];
    function.blocks[entry].terminator = Terminator::Br(success);
    let record = function.temps.alloc(Temp {
        ty: LirType::ExceptionRecord,
    });
    let raw = function.temps.alloc(Temp { ty: RAW_PTR });
    let payload = function.temps.alloc(Temp { ty: MANAGED_PTR });
    let mut instructions = vec![
        Instruction::LandingPad { record, raw },
        Instruction::BeginCatch {
            out: payload,
            raw: Value::Temp(raw),
        },
    ];
    if let Some(global) = failure_root {
        let out = function.temps.alloc(Temp { ty: MANAGED_PTR });
        let reference = AbiValue::new(
            MANAGED_PTR,
            AbiNonZeroLayout::new(8, 8).unwrap(),
            RefScan::References(vec![0]),
        )
        .unwrap();
        let signature = targets.direct_signatures.alloc(DirectCallSignature::new(
            vec![AbiArgument::Direct(reference.clone())],
            reference,
            CallingConvention::Cdecl,
        ));
        let target = targets.managed_targets.direct.alloc(CallTarget {
            destination: ManagedCallDestination::Runtime(
                ManagedRuntimeFunction::MaterializeException,
            ),
            signature,
        });
        identities.push((
            SafepointSiteRef::from_u32(2),
            SafepointIdentity::new(
                function.callable_body.id(),
                SafepointSiteRole::ManagedCall,
                0,
            )
            .unwrap(),
        ));
        instructions.push(Instruction::Call {
            site: CallSite::Managed(ManagedCallSite {
                call: ManagedTypedCall::Direct {
                    target,
                    out,
                    args: vec![AbiCallArgument::Direct(Value::Temp(payload))],
                },
                safepoint: SafepointSiteRef::from_u32(2),
                live: StatepointLiveSet::new(vec![StatepointLiveValue {
                    source: CallerRootSource::Temp(payload),
                    ty: MANAGED_PTR,
                    leaves: ManagedLeafPaths::new(vec![ManagedLeafPath { byte_offset: 0 }])
                        .unwrap(),
                }])
                .unwrap(),
            }),
        });
        instructions.push(Instruction::GlobalStore {
            global,
            value: Value::Temp(out),
        });
    }
    instructions.push(Instruction::EndCatch);
    function.blocks[failure].instructions = instructions;
    let reference = AbiValue::new(
        MANAGED_PTR,
        AbiNonZeroLayout::new(8, 8).unwrap(),
        RefScan::References(vec![0]),
    )
    .unwrap();
    let metadata = AbiValue::new(
        METADATA_PTR,
        AbiNonZeroLayout::new(8, 8).unwrap(),
        RefScan::None,
    )
    .unwrap();
    let signature = targets.direct_signatures.alloc(DirectCallSignature::new(
        vec![AbiArgument::Direct(metadata)],
        reference,
        CallingConvention::Cdecl,
    ));
    let target = targets.managed_targets.direct.alloc(CallTarget {
        destination: ManagedCallDestination::Runtime(ManagedRuntimeFunction::ContextEnsureRoot),
        signature,
    });
    let task = function.temps.alloc(Temp { ty: MANAGED_PTR });
    let safepoint = SafepointSiteRef::from_u32(3);
    identities.push((
        safepoint,
        SafepointIdentity::new(
            function.callable_body.id(),
            SafepointSiteRole::ManagedCall,
            1,
        )
        .unwrap(),
    ));
    function.blocks[entry].instructions.insert(
        1,
        Instruction::Call {
            site: CallSite::Managed(ManagedCallSite {
                call: ManagedTypedCall::Direct {
                    target,
                    out: task,
                    args: vec![AbiCallArgument::Direct(Value::TypeDescriptor(
                        task_descriptor,
                    ))],
                },
                safepoint,
                live: StatepointLiveSet::default(),
            }),
        },
    );
    function.safepoints = SafepointIdentities::checked(identities).unwrap();
    function
}
fn exit(status: u32) -> BasicBlock {
    BasicBlock {
        name: format!("exit{status}"),
        instructions: vec![],
        terminator: Terminator::Return {
            value: Some(Value::IntegerConst(LirIntegerConstant::Unsigned32(status))),
        },
    }
}
