use super::*;

#[test]
fn root_failure_publishes_a_rooted_managed_copy_before_ending_the_catch() {
    let module = lower(hello_world());
    let gateway = module.functions.last().unwrap();
    assert!(matches!(
        gateway.blocks[gateway.entry].instructions.first(),
        Some(lir::Instruction::ManagedPoll { .. })
    ));
    let failure = gateway
        .blocks
        .iter()
        .map(|(_, block)| block)
        .find(|block| block.name == "failure")
        .unwrap();
    let [
        lir::Instruction::LandingPad { .. },
        lir::Instruction::BeginCatch { out: payload, .. },
        lir::Instruction::Call {
            site: lir::CallSite::Managed(site),
        },
        lir::Instruction::GlobalStore { global, value },
        lir::Instruction::EndCatch,
    ] = failure.instructions.as_slice()
    else {
        panic!("root failure must materialize before publishing and ending the catch");
    };
    let lir::ManagedTypedCall::Direct { target, out, args } = &site.call else {
        panic!("materialization returns a managed reference");
    };
    assert_eq!(
        gateway.call_targets.managed_targets.direct[*target].destination,
        lir::ManagedCallDestination::runtime(lir::ManagedRuntimeFunction::MaterializeException)
    );
    assert_eq!(
        args,
        &[lir::AbiCallArgument::Direct(lir::Value::Temp(*payload))]
    );
    assert_ne!(out, payload, "the native payload cannot escape EndCatch");
    assert_eq!(*value, lir::Value::Temp(*out));
    assert_eq!(
        module.globals[*global].scan,
        lir::RefScan::References(vec![0])
    );
    assert_eq!(site.live.as_slice().len(), 1);
    assert_eq!(
        site.live.as_slice()[0].source,
        lir::CallerRootSource::Temp(*payload)
    );
    assert_eq!(
        site.live.as_slice()[0].leaves.as_slice(),
        &[lir::ManagedLeafPath { byte_offset: 0 }]
    );
    assert!(matches!(
        failure.terminator,
        lir::Terminator::Return {
            value: Some(lir::Value::IntegerConst(
                lir::LirIntegerConstant::Unsigned32(1)
            ))
        }
    ));
}
