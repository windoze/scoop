use super::*;

#[test]
fn root_failure_publishes_the_original_exception_before_ending_the_catch() {
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
        lir::Instruction::GlobalStore { global, value },
        lir::Instruction::EndCatch,
    ] = failure.instructions.as_slice()
    else {
        panic!("root failure must publish the original exception before ending the catch");
    };
    assert_eq!(*value, lir::Value::Temp(*payload));
    assert_eq!(gateway.temps[*payload].ty, lir::MANAGED_PTR);
    assert_eq!(
        module.globals[*global].scan,
        lir::RefScan::References(vec![0])
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
