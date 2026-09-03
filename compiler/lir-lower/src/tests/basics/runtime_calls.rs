use super::*;

#[test]
fn compiler_runtime_calls_with_results_produce_typed_temps() {
    let mut b = Builder::new();
    let s0 = b.string("a");
    let s1 = b.string("b");
    let helper = b.user_fn("helper", "scoop.helper", Arena::new(), vec![]);
    let mut locals = Arena::new();
    let s = locals.alloc(local("s", mir::Type::String));
    let main = b.main(
        locals,
        vec![
            call_value(
                s,
                runtime_call(
                    mir::RuntimeFn::StringConcat,
                    vec![string_expr(s0), string_expr(s1)],
                ),
            ),
            call_stmt(user_call(helper)),
        ],
    );
    let module = lower(&b.finish(main));

    // top_level order: helper first, then main.
    let function = &module.functions[1];
    let instructions = instructions_without_polls(&function.blocks[function.entry]);

    let lir::Instruction::Call { site } = instructions[0] else {
        panic!("string concat must produce a value")
    };
    let concat_out = site.direct_out().expect("string concat result");
    assert_eq!(
        call_symbol(&module, site.destination(&function.call_targets)),
        "scoop_rt_string_concat"
    );
    assert_eq!(function.temps[concat_out].ty, lir::MANAGED_PTR);
    assert!(matches!(instructions[1], lir::Instruction::Store { .. }));

    // User calls return void; the Unit value is a fresh empty
    // aggregate.
    let lir::Instruction::Call { site } = instructions[2] else {
        panic!("user calls must return void")
    };
    assert_eq!(site.result(), lir::TypedCallResult::Void);
    assert_eq!(
        call_symbol(&module, site.destination(&function.call_targets)),
        "scoop.helper"
    );
    let lir::Instruction::MakeAggregate { out, elements } = instructions[3] else {
        panic!("a void call's Unit value must be an empty aggregate")
    };
    assert!(elements.is_empty());
    assert_eq!(function.temps[*out].ty, lir::LirType::Aggregate(Vec::new()));
}

#[test]
fn pointer_nulls_preserve_raw_and_code_provenance_in_lir() {
    assert!(matches!(
        lower_constant(&mir::ConstantValue::NullPtr),
        lir::ConstantValue::NullPointer(lir::PointerKind::Raw)
    ));
    assert!(matches!(
        lower_constant(&mir::ConstantValue::NullFunPtr),
        lir::ConstantValue::NullPointer(lir::PointerKind::Code)
    ));

    let mut blocks = Arena::new();
    let entry = blocks.alloc(lir::BasicBlock {
        name: "entry".to_string(),
        instructions: Vec::new(),
        terminator: lir::Terminator::Return { value: None },
    });
    let function = lir::Function {
        gc_effect: lir::GcEffect::NoGc,
        symbol: "null_provenance".to_string(),
        params: Vec::new(),
        return_ty: lir::LirType::Void,
        call_targets: lir::CallTargets::default(),
        locals: Arena::new(),
        temps: Arena::new(),
        blocks,
        entry,
    };
    let globals = Arena::new();
    for kind in [
        lir::PointerKind::Managed,
        lir::PointerKind::Raw,
        lir::PointerKind::Code,
        lir::PointerKind::Metadata,
    ] {
        assert_eq!(
            function.value_ty(&globals, lir::Value::NullPointer(kind)),
            lir::LirType::Ptr(kind)
        );
    }
}
