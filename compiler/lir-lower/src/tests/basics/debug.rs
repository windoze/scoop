use super::*;
use std::sync::Arc;

#[test]
fn source_locations_stay_with_managed_calls_when_polls_are_inserted() {
    let mut source = hello_world();
    let file = Arc::new(mir::DebugFile {
        identity: Arc::new(scoop_identity::SourceIdentity::single_file()),
        path: "/workspace/hello.scoop".into(),
    });
    let mut records = mir::DebugRecords::default();
    let definition = records.source(file.clone(), 1, 1);
    let call = records.source(file.clone(), 2, 5);
    let returned = records.source(file, 3, 1);
    let helper = source.top_level[0];
    source.functions[helper].debug_origin = definition.clone();
    let entry = source.functions[helper].body.entry;
    let block = &mut source.functions[helper].body.blocks[entry];
    block.statements[0].debug_origin = call.clone();
    block.terminator_origin = returned.clone();

    let module = lower(source);
    let function = &module.functions[0];
    assert_eq!(function.debug_name, "helper");
    assert_eq!(function.debug_origin, definition);
    let block = &function.blocks[function.entry];
    assert!(matches!(
        block.instructions[0],
        lir::Instruction::ManagedPoll { .. }
    ));
    assert_eq!(block.instructions.origin(0), &mir::DebugOrigin::Generated);
    let index = block
        .instructions
        .iter()
        .position(|instruction| matches!(instruction, lir::Instruction::Call { .. }))
        .unwrap();
    let (mir::DebugOrigin::Source(expected), mir::DebugOrigin::Source(actual)) =
        (&call, block.instructions.origin(index))
    else {
        panic!("the source call retains its observation");
    };
    assert!(Arc::ptr_eq(expected, actual));
    assert_eq!(block.terminator_origin, returned);
}
