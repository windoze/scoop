use super::variables::{bind, origin};
use super::*;
use scoop_lir::{DebugEncoding, DebugStorage, DebugType, DebugTypeKind};

#[test]
fn observations_added_to_a_terminated_block_precede_its_terminator() {
    let mut module = values_module();
    let exact = test_exact_type("debug.Long");
    module.meta.debug_types.insert(
        exact,
        DebugType {
            name: "Long".into(),
            size: 8,
            align: 8,
            kind: DebugTypeKind::Basic(DebugEncoding::Signed),
        },
    );
    let function = &mut module.functions[0];
    function.debug_origin = origin(1);
    let local = function.locals.alloc(test_local("late", LirType::I64));
    let storage = DebugStorage::Local(local);
    bind(function, "late", exact, storage, None);

    let context = Context::create();
    let llvm = context.create_module("late-debug-observations");
    let compiled = llvm.add_function(
        function.symbol(),
        context.void_type().fn_type(&[], false),
        None,
    );
    let block = context.append_basic_block(compiled, "cleanup");
    let builder = context.create_builder();
    builder.position_at_end(block);
    let slot = builder.build_alloca(context.i64_type(), "late").unwrap();
    let value = context.i64_type().const_int(42, false);
    builder.build_store(slot, value).unwrap();
    builder.build_return(None).unwrap();

    let debug = crate::debug::DebugContext::new(
        &context,
        &llvm,
        host_profile().with_debug_info(scoop_lir::DebugInfoLevel::Full),
        crate::debug::source_file(function).unwrap(),
        &module.meta.debug_types,
    );
    let mut observations = debug.function(function, compiled).unwrap();
    observations.register(function.debug_variables.last().unwrap(), false);
    observations.declare(storage, slot, block);
    observations.value(storage, value.into(), None, block);
    debug.finish();
    llvm.verify().unwrap();

    let text = llvm.print_to_string().to_string();
    let declaration = text.find("#dbg_declare(").expect("declare is attached");
    let observation = text.find("#dbg_value(").expect("value is attached");
    let terminator = text.find("ret void").unwrap();
    assert!(
        declaration < observation && observation < terminator,
        "{text}"
    );
}
