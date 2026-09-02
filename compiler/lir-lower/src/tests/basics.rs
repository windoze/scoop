use super::*;

/// `main` writes `"hello, world"` (core's managed `write` extern)
/// then calls `helper()`, which writes `"!"`.
fn hello_world() -> mir::Module {
    let mut b = Builder::new();
    let hello = b.string("hello, world");
    let bang = b.string("!");
    let write = b.managed_scoop_extern(
        "write",
        "scoop_rt_write",
        vec![mir::Type::String],
        mir::Type::Unit,
    );
    let helper = b.user_fn(
        "helper",
        "scoop.helper",
        Arena::new(),
        vec![call_stmt(extern_call(write, vec![string_expr(bang)]))],
    );
    let main = b.main(
        Arena::new(),
        vec![
            call_stmt(extern_call(write, vec![string_expr(hello)])),
            call_stmt(user_call(helper)),
        ],
    );
    b.finish(main)
}

#[test]
fn lowers_hello_world() {
    let module = lower(&hello_world());

    // Globals: one per MIR string constant, same symbol and value.
    let globals: Vec<(&str, &str)> = module
        .globals
        .iter()
        .map(|(_, g)| match &g.init {
            lir::GlobalInit::StringConst(value) => (g.symbol.as_str(), value.as_str()),
            lir::GlobalInit::CString(value) => (g.symbol.as_str(), value.as_str()),
            lir::GlobalInit::Storage { .. } => unreachable!("hello has no storage globals"),
        })
        .collect();
    assert_eq!(
        globals,
        [("scoop.str.0", "hello, world"), ("scoop.str.1", "!")]
    );

    // Functions keep their mangled symbols; the entry symbol is the
    // fixed `scoop_main`.
    let symbols: Vec<&str> = module.functions.iter().map(|f| f.symbol.as_str()).collect();
    assert_eq!(symbols, ["scoop.helper", mir::ENTRY_SYMBOL]);
    assert_eq!(module.entry_symbol, mir::ENTRY_SYMBOL);

    // The source declaration's typed intrinsic identity survives through
    // MIR and LIR. String metadata is a required singleton, not a layout
    // or descriptor that codegen has to rediscover by name.
    let string_layout = &module.meta.layouts[module.meta.well_known_layouts.string];
    let string_descriptor = descriptor(&module, module.meta.well_known_type_descriptors.string);
    assert_eq!(
        string_layout.kind,
        lir::LayoutKind::Intrinsic(lir::IntrinsicTypeRepresentation::String)
    );
    assert_eq!(string_descriptor.symbol, lir::STRING_TD_SYMBOL);
    assert_eq!(string_descriptor.runtime_type_id, 1);
    assert!(string_descriptor.vtable.is_empty());
    assert_eq!(
        descriptor_values(&module)
            .filter(|descriptor| descriptor.symbol == lir::STRING_TD_SYMBOL)
            .count(),
        1
    );
    for representation in [
        lir::IntrinsicTypeRepresentation::Int,
        lir::IntrinsicTypeRepresentation::UInt,
        lir::IntrinsicTypeRepresentation::Boolean,
    ] {
        assert!(
            layout_values(&module).any(|layout| {
                layout.kind == lir::LayoutKind::Intrinsic(representation.clone())
            })
        );
    }

    // Golden dump locks the output structure.
    insta::assert_snapshot!(lir::dump(&module), @r###"
Module
  global @scoop.str.0 = "hello, world"
  global @scoop.str.1 = "!"
  extern ef0 write @scoop_rt_write(ptr<managed>) -> {} <scoop managed nounwind>
  fun @scoop.helper() -> void
  block entry
    poll managed-void-target0 sp4 live=[]
    call native-borrowed-void-target0 sp1 roots=[] sig=void0 (ptr<managed>) extern0(global1)
    t0 = aggregate () : {}
    ret
  fun @scoop_main() -> void
  block entry
    poll managed-void-target1 sp5 live=[]
    call native-borrowed-void-target0 sp2 roots=[] sig=void0 (ptr<managed>) extern0(global0)
    t0 = aggregate () : {}
    call managed-void-target0 sp3 live=[] sig=void1 () local-fn0()
    t1 = aggregate () : {}
    ret
  layout String size=24 align=8 refs=[]
  layout Int size=8 align=8 refs=[]
  layout Boolean size=1 align=1 refs=[]
  entry @scoop_main
"###);
}

#[test]
fn globals_carry_complete_scans_from_their_concrete_storage_types() {
    let mut module = hello_world();
    module.globals.alloc(mir::Global {
        name: "managedRoot".to_string(),
        symbol: "scoop.global.managedRoot".to_string(),
        ty: mir::Type::String,
        mutable: true,
        storage: mir::GlobalStorage::Local {
            thread_local: false,
            initializer: mir::ConstantValue::NullPtr,
        },
    });

    let module = lower(&module);
    let string_constant = module
        .globals
        .iter()
        .map(|(_, global)| global)
        .find(|global| matches!(global.init, lir::GlobalInit::StringConst(_)))
        .expect("string constant");
    assert_eq!(string_constant.scan, lir::RefScan::None);

    let managed = module
        .globals
        .iter()
        .map(|(_, global)| global)
        .find(|global| global.symbol == "scoop.global.managedRoot")
        .expect("managed storage global");
    assert_eq!(managed.scan, lir::RefScan::References(vec![0]));
    assert!(
        lir::dump(&module).contains("global @scoop.global.managedRoot : ptr<managed> scan=refs[0]")
    );
}

#[test]
fn if_else_becomes_basic_blocks() {
    let mut b = Builder::new();
    let ok = b.string("ok");
    let ng = b.string("ng");
    let write = b.managed_scoop_extern(
        "write",
        "scoop_rt_write",
        vec![mir::Type::String],
        mir::Type::Unit,
    );
    let mut blocks = Arena::new();
    let entry = cfg_block(&mut blocks, "entry");
    let then_block = cfg_block(&mut blocks, "if.then.1");
    let else_block = cfg_block(&mut blocks, "if.else.2");
    let merge = cfg_block(&mut blocks, "if.merge.3");
    set_cfg_block(
        &mut blocks,
        entry,
        Vec::new(),
        mir::Terminator::Branch {
            cond: mir::Expr::bool(true),
            then_block,
            else_block,
        },
        None,
    );
    set_cfg_block(
        &mut blocks,
        then_block,
        vec![call_stmt(extern_call(write, vec![string_expr(ok)]))],
        mir::Terminator::Goto(merge),
        None,
    );
    set_cfg_block(
        &mut blocks,
        else_block,
        vec![call_stmt(extern_call(write, vec![string_expr(ng)]))],
        mir::Terminator::Goto(merge),
        None,
    );
    set_cfg_block(
        &mut blocks,
        merge,
        Vec::new(),
        mir::Terminator::Return { value: None },
        None,
    );
    let main = b.user_fn_body(
        "main",
        mir::ENTRY_SYMBOL,
        Vec::new(),
        mir::Type::Unit,
        mir::Body {
            locals: Arena::new(),
            blocks,
            entry,
        },
    );
    let module = lower(&b.finish(main));

    insta::assert_snapshot!(lir::dump(&module), @r###"
Module
  global @scoop.str.0 = "ok"
  global @scoop.str.1 = "ng"
  extern ef0 write @scoop_rt_write(ptr<managed>) -> {} <scoop managed nounwind>
  fun @scoop_main() -> void
  block entry
    poll managed-void-target0 sp3 live=[]
    br @if.then.1
  block if.then.1
    call native-borrowed-void-target0 sp1 roots=[] sig=void0 (ptr<managed>) extern0(global0)
    t0 = aggregate () : {}
    br @if.merge.3
  block if.merge.3
    ret
  layout String size=24 align=8 refs=[]
  layout Int size=8 align=8 refs=[]
  layout Boolean size=1 align=1 refs=[]
  entry @scoop_main
"###);
}

#[test]
fn while_becomes_basic_blocks() {
    // var n = 0; while (n < 3) { n = n + 1 }
    let mut b = Builder::new();
    let mut locals = Arena::new();
    let n = locals.alloc(var("n", mir::Type::Int));
    let mut blocks = Arena::new();
    let entry = cfg_block(&mut blocks, "entry");
    let cond = cfg_block(&mut blocks, "while.cond.1");
    let body = cfg_block(&mut blocks, "while.body.2");
    let exit = cfg_block(&mut blocks, "while.exit.3");
    set_cfg_block(
        &mut blocks,
        entry,
        vec![val_decl(n, mir::Expr::int(0))],
        mir::Terminator::Goto(cond),
        None,
    );
    set_cfg_block(
        &mut blocks,
        cond,
        Vec::new(),
        mir::Terminator::Branch {
            cond: binary(
                mir::BinOp::IntLt,
                local_expr(n, mir::Type::Int),
                mir::Expr::int(3),
                mir::Type::Boolean,
            ),
            then_block: body,
            else_block: exit,
        },
        None,
    );
    set_cfg_block(
        &mut blocks,
        body,
        vec![assign(
            n,
            binary(
                mir::BinOp::IntAdd,
                local_expr(n, mir::Type::Int),
                mir::Expr::int(1),
                mir::Type::Int,
            ),
        )],
        mir::Terminator::Goto(cond),
        None,
    );
    set_cfg_block(
        &mut blocks,
        exit,
        Vec::new(),
        mir::Terminator::Return { value: None },
        None,
    );
    let main = b.user_fn_body(
        "main",
        mir::ENTRY_SYMBOL,
        Vec::new(),
        mir::Type::Unit,
        mir::Body {
            locals,
            blocks,
            entry,
        },
    );
    let module = lower(&b.finish(main));

    insta::assert_snapshot!(lir::dump(&module), @r###"
Module
  fun @scoop_main() -> void
    local %0 n: i64
  block entry
    poll managed-void-target0 sp1 live=[]
    store 0 -> local0
    br @while.cond.1
  block while.cond.1
    poll managed-void-target0 sp2 live=[]
    t0 = Lt local0, 3 : i1
    cbr t0 then @while.body.2 else @while.exit.3
  block while.body.2
    t1 = Add local0, 1 : i64
    store t1 -> local0
    br @while.cond.1
  block while.exit.3
    ret
  layout String size=24 align=8 refs=[]
  layout Int size=8 align=8 refs=[]
  layout Boolean size=1 align=1 refs=[]
  entry @scoop_main
"###);
}

#[test]
fn and_short_circuits_through_blocks() {
    // val b = eq(s0, s1) && eq(s2, s3): the second comparison call
    // sits in its own block, executed only when the first is true.
    let mut b = Builder::new();
    let s0 = b.string("a");
    let s1 = b.string("b");
    let s2 = b.string("c");
    let s3 = b.string("d");
    let string_eq = b.managed_scoop_extern(
        "coreStringEquals",
        "scoop_rt_string_eq",
        vec![mir::Type::String, mir::Type::String],
        mir::Type::Boolean,
    );
    let string_eq = |l, r| extern_call(string_eq, vec![string_expr(l), string_expr(r)]);
    let mut locals = Arena::new();
    let lhs = locals.alloc(local("$call.1", mir::Type::Boolean));
    let rhs = locals.alloc(local("$call.2", mir::Type::Boolean));
    let result = locals.alloc(local("b", mir::Type::Boolean));
    let mut blocks = Arena::new();
    let entry = cfg_block(&mut blocks, "entry");
    let rhs_block = cfg_block(&mut blocks, "logic.rhs.1");
    let short_block = cfg_block(&mut blocks, "logic.short.2");
    let merge = cfg_block(&mut blocks, "logic.merge.3");
    set_cfg_block(
        &mut blocks,
        entry,
        vec![call_value(lhs, string_eq(s0, s1))],
        mir::Terminator::Branch {
            cond: local_expr(lhs, mir::Type::Boolean),
            then_block: rhs_block,
            else_block: short_block,
        },
        None,
    );
    set_cfg_block(
        &mut blocks,
        rhs_block,
        vec![
            call_value(rhs, string_eq(s2, s3)),
            assign(result, local_expr(rhs, mir::Type::Boolean)),
        ],
        mir::Terminator::Goto(merge),
        None,
    );
    set_cfg_block(
        &mut blocks,
        short_block,
        vec![assign(result, mir::Expr::bool(false))],
        mir::Terminator::Goto(merge),
        None,
    );
    set_cfg_block(
        &mut blocks,
        merge,
        Vec::new(),
        mir::Terminator::Return { value: None },
        None,
    );
    let main = b.user_fn_body(
        "main",
        mir::ENTRY_SYMBOL,
        Vec::new(),
        mir::Type::Unit,
        mir::Body {
            locals,
            blocks,
            entry,
        },
    );
    let module = lower(&b.finish(main));

    insta::assert_snapshot!(lir::dump(&module), @r###"
Module
  global @scoop.str.0 = "a"
  global @scoop.str.1 = "b"
  global @scoop.str.2 = "c"
  global @scoop.str.3 = "d"
  extern ef0 coreStringEquals @scoop_rt_string_eq(ptr<managed>, ptr<managed>) -> i1 <scoop managed nounwind>
  fun @scoop_main() -> void
    local %0 $call.1: i1
    local %1 $call.2: i1
    local %2 b: i1
  block entry
    poll managed-void-target0 sp3 live=[]
    call native-borrowed-direct-target0 sp1 roots=[] t0 = sig=direct0 (ptr<managed>, ptr<managed>) -> i1 extern0(global0, global1)
    store t0 -> local0
    cbr local0 then @logic.rhs.1 else @logic.short.2
  block logic.rhs.1
    call native-borrowed-direct-target1 sp2 roots=[] t1 = sig=direct1 (ptr<managed>, ptr<managed>) -> i1 extern0(global2, global3)
    store t1 -> local1
    store local1 -> local2
    br @logic.merge.3
  block logic.short.2
    store false -> local2
    br @logic.merge.3
  block logic.merge.3
    ret
  layout String size=24 align=8 refs=[]
  layout Int size=8 align=8 refs=[]
  layout Boolean size=1 align=1 refs=[]
  entry @scoop_main
"###);
}

#[test]
fn or_short_circuits_through_blocks() {
    // val b = x || y: when x is true, y is never evaluated.
    let mut b = Builder::new();
    let mut locals = Arena::new();
    let x = locals.alloc(local("x", mir::Type::Boolean));
    let y = locals.alloc(local("y", mir::Type::Boolean));
    let result = locals.alloc(local("b", mir::Type::Boolean));
    let mut blocks = Arena::new();
    let entry = cfg_block(&mut blocks, "entry");
    let rhs = cfg_block(&mut blocks, "logic.rhs.1");
    let short = cfg_block(&mut blocks, "logic.short.2");
    let merge = cfg_block(&mut blocks, "logic.merge.3");
    set_cfg_block(
        &mut blocks,
        entry,
        vec![
            val_decl(x, mir::Expr::bool(true)),
            val_decl(y, mir::Expr::bool(false)),
        ],
        mir::Terminator::Branch {
            cond: local_expr(x, mir::Type::Boolean),
            then_block: short,
            else_block: rhs,
        },
        None,
    );
    set_cfg_block(
        &mut blocks,
        rhs,
        vec![assign(result, local_expr(y, mir::Type::Boolean))],
        mir::Terminator::Goto(merge),
        None,
    );
    set_cfg_block(
        &mut blocks,
        short,
        vec![assign(result, mir::Expr::bool(true))],
        mir::Terminator::Goto(merge),
        None,
    );
    set_cfg_block(
        &mut blocks,
        merge,
        Vec::new(),
        mir::Terminator::Return { value: None },
        None,
    );
    let main = b.user_fn_body(
        "main",
        mir::ENTRY_SYMBOL,
        Vec::new(),
        mir::Type::Unit,
        mir::Body {
            locals,
            blocks,
            entry,
        },
    );
    let module = lower(&b.finish(main));

    insta::assert_snapshot!(lir::dump(&module), @r###"
Module
  fun @scoop_main() -> void
    local %0 x: i1
    local %1 y: i1
    local %2 b: i1
  block entry
    poll managed-void-target0 sp1 live=[]
    store true -> local0
    store false -> local1
    cbr local0 then @logic.short.2 else @logic.rhs.1
  block logic.rhs.1
    store local1 -> local2
    br @logic.merge.3
  block logic.short.2
    store true -> local2
    br @logic.merge.3
  block logic.merge.3
    ret
  layout String size=24 align=8 refs=[]
  layout Int size=8 align=8 refs=[]
  layout Boolean size=1 align=1 refs=[]
  entry @scoop_main
"###);
}

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

#[test]
fn arithmetic_and_comparison_ops_map_to_lir_ops() {
    let mut b = Builder::new();
    let int_cases = [
        (mir::BinOp::IntAdd, lir::BinOp::Add, lir::LirType::I64),
        (mir::BinOp::IntSub, lir::BinOp::Sub, lir::LirType::I64),
        (mir::BinOp::IntMul, lir::BinOp::Mul, lir::LirType::I64),
        (mir::BinOp::IntDiv, lir::BinOp::SDiv, lir::LirType::I64),
        (mir::BinOp::IntLt, lir::BinOp::Lt, lir::LirType::I1),
        (mir::BinOp::IntLe, lir::BinOp::Le, lir::LirType::I1),
        (mir::BinOp::IntGt, lir::BinOp::Gt, lir::LirType::I1),
        (mir::BinOp::IntGe, lir::BinOp::Ge, lir::LirType::I1),
        (mir::BinOp::IntEq, lir::BinOp::Eq, lir::LirType::I1),
        (mir::BinOp::IntNe, lir::BinOp::Ne, lir::LirType::I1),
    ];
    let mut statements: Vec<mir::Statement> = int_cases
        .iter()
        .map(|(mir_op, _, _)| {
            expr_stmt(binary(
                *mir_op,
                mir::Expr::int(1),
                mir::Expr::int(2),
                if matches!(
                    mir_op,
                    mir::BinOp::IntAdd
                        | mir::BinOp::IntSub
                        | mir::BinOp::IntMul
                        | mir::BinOp::IntDiv
                ) {
                    mir::Type::Int
                } else {
                    mir::Type::Boolean
                },
            ))
        })
        .collect();
    let bool_cases = [
        (mir::BinOp::BoolEq, lir::BinOp::Eq, lir::LirType::I1),
        (mir::BinOp::BoolNe, lir::BinOp::Ne, lir::LirType::I1),
    ];
    for (mir_op, _, _) in &bool_cases {
        statements.push(expr_stmt(binary(
            *mir_op,
            mir::Expr::bool(true),
            mir::Expr::bool(false),
            mir::Type::Boolean,
        )));
    }
    let main = b.main(Arena::new(), statements);
    let module = lower(&b.finish(main));

    let expected: Vec<(lir::BinOp, lir::LirType)> = int_cases
        .iter()
        .chain(bool_cases.iter())
        .map(|(_, lir_op, ty)| (*lir_op, ty.clone()))
        .collect();
    let function = &module.functions[0];
    let ops: Vec<(lir::BinOp, lir::LirType)> = function.blocks[function.entry]
        .instructions
        .iter()
        .filter_map(|instruction| {
            let lir::Instruction::BinOp { out, op, .. } = instruction else {
                return None;
            };
            Some((*op, function.temps[*out].ty.clone()))
        })
        .collect();
    assert_eq!(ops, expected);
}

#[test]
fn unary_ops_map_to_lir_unops() {
    let mut b = Builder::new();
    let main = b.main(
        Arena::new(),
        vec![
            expr_stmt(expr(
                mir::Type::Int,
                mir::ExprKind::Unary {
                    op: mir::UnOp::IntNeg,
                    operand: Box::new(mir::Expr::int(1)),
                },
            )),
            expr_stmt(expr(
                mir::Type::Boolean,
                mir::ExprKind::Unary {
                    op: mir::UnOp::BoolNot,
                    operand: Box::new(mir::Expr::bool(true)),
                },
            )),
        ],
    );
    let module = lower(&b.finish(main));

    let function = &module.functions[0];
    let ops: Vec<(lir::UnOp, lir::LirType)> = function.blocks[function.entry]
        .instructions
        .iter()
        .filter_map(|instruction| {
            let lir::Instruction::UnaryOp { out, op, .. } = instruction else {
                return None;
            };
            Some((*op, function.temps[*out].ty.clone()))
        })
        .collect();
    assert_eq!(
        ops,
        [
            (lir::UnOp::Neg, lir::LirType::I64),
            (lir::UnOp::Not, lir::LirType::I1),
        ]
    );
}

#[test]
fn struct_values_keep_named_lir_identity() {
    let mut b = Builder::new();
    let point = b.strukt("Point", &[("x", mir::Type::Int), ("y", mir::Type::Int)]);
    let mut locals = Arena::new();
    let p = locals.alloc(local("p", mir::Type::Struct(point)));
    let x = locals.alloc(local("x", mir::Type::Int));
    let main = b.main(
        locals,
        vec![
            val_decl(
                p,
                expr(
                    mir::Type::Struct(point),
                    mir::ExprKind::StructInit {
                        struct_id: point,
                        args: vec![mir::Expr::int(1), mir::Expr::int(2)],
                    },
                ),
            ),
            val_decl(
                x,
                expr(
                    mir::Type::Int,
                    mir::ExprKind::FieldAccess {
                        receiver: Box::new(local_expr(p, mir::Type::Struct(point))),
                        index: 0,
                    },
                ),
            ),
        ],
    );
    let module = lower(&b.finish(main));

    let function = &module.functions[0];
    let local_types: Vec<lir::LirType> =
        function.locals.iter().map(|(_, l)| l.ty.clone()).collect();
    assert_eq!(
        local_types,
        [
            lir::LirType::Struct(struct_def_id(point)),
            lir::LirType::I64,
        ]
    );

    let instructions = instructions_without_polls(&function.blocks[function.entry]);
    let lir::Instruction::MakeAggregate { out, elements } = instructions[0] else {
        panic!("struct construction must build an aggregate")
    };
    assert_eq!(elements.len(), 2);
    assert_eq!(
        function.temps[*out].ty,
        lir::LirType::Struct(struct_def_id(point))
    );
    assert!(matches!(instructions[1], lir::Instruction::Store { .. }));
    let lir::Instruction::ExtractValue { out, index, .. } = instructions[2] else {
        panic!("field access must extract from the aggregate")
    };
    assert_eq!(*index, 0);
    assert_eq!(function.temps[*out].ty, lir::LirType::I64);
    assert!(matches!(instructions[3], lir::Instruction::Store { .. }));

    // The struct layout is in the meta.
    let layout = layout_values(&module)
        .find(|l| l.name == "Point")
        .expect("a layout per struct");
    assert_eq!((layout.size, layout.align), (16, 8));
    assert!(plain_refs(layout).is_empty());
}

#[test]
fn unit_is_the_empty_aggregate() {
    let mut b = Builder::new();
    let mut locals = Arena::new();
    let u = locals.alloc(local("u", mir::Type::Unit));
    let main = b.main(locals, vec![val_decl(u, mir::Expr::unit())]);
    let module = lower(&b.finish(main));

    let function = &module.functions[0];
    let (_, u_local) = function.locals.iter().next().expect("one local");
    assert_eq!(u_local.ty, lir::LirType::Aggregate(Vec::new()));
    let instructions = instructions_without_polls(&function.blocks[function.entry]);
    let lir::Instruction::MakeAggregate { out, elements } = instructions[0] else {
        panic!("Unit must be an empty aggregate")
    };
    assert!(elements.is_empty());
    assert_eq!(function.temps[*out].ty, lir::LirType::Aggregate(Vec::new()));

    // Unit itself gets no layout entry (it is just `{}`).
    assert!(!layout_values(&module).any(|l| l.name == "Unit"));
}

#[test]
fn uint_shares_ints_machine_word() {
    // UInt (spec 11.2, M9) maps onto `i64` at LIR — the same
    // machine word as Int, so codegen needs no UInt-specific
    // handling.
    let mut b = Builder::new();
    // A UInt field in a class: an 8-byte scalar slot behind the
    // 16-byte header, exactly like an Int field.
    let _c = b.class("C", None, &[("u", mir::Type::UInt)], empty_vtable(), vec![]);
    let mut locals = Arena::new();
    let u = locals.alloc(local("u", mir::Type::UInt));
    let main = b.main(locals, vec![val_decl(u, mir::Expr::int(1))]);
    let module = lower(&b.finish(main));

    let function = &module.functions[0];
    let (_, u_local) = function.locals.iter().next().expect("one local");
    assert_eq!(u_local.ty, lir::LirType::I64);

    let c_layout = layout_values(&module)
        .find(|l| l.name == "C")
        .expect("a layout per class");
    assert_eq!((c_layout.size, c_layout.align), (24, 8));
    assert!(plain_refs(c_layout).is_empty());
    let c_td = descriptor_values(&module)
        .find(|td| td.name == "C")
        .expect("a TypeDescriptor per class");
    assert_eq!(c_td.size, 24);
    assert_eq!(*fixed_scan(c_td), lir::RefScan::None);
}

#[test]
fn layouts_mark_reference_fields_for_the_gc() {
    let mut b = Builder::new();
    // String field behind one Int: the reference sits at offset 8.
    let s = b.strukt("S", &[("a", mir::Type::Int), ("s", mir::Type::String)]);
    // A String nested inside a tuple field, after a Boolean: the
    // tuple is 8-aligned, so it starts at offset 8.
    let pair = mir::Type::Tuple(vec![mir::Type::String, mir::Type::Int]);
    let _outer = b.strukt("Outer", &[("flag", mir::Type::Boolean), ("pair", pair)]);
    let _ = s;
    // A tuple type that only appears in code (padding: Boolean
    // then Int at offset 8).
    let mut locals = Arena::new();
    let _t = locals.alloc(local(
        "t",
        mir::Type::Tuple(vec![mir::Type::Boolean, mir::Type::Int]),
    ));
    let main = b.main(locals, vec![]);
    let module = lower(&b.finish(main));

    let by_name = |name: &str| {
        layout_values(&module)
            .find(|l| l.name == name)
            .unwrap_or_else(|| panic!("missing layout for {name}"))
    };

    // The String singleton is structurally separate; the remaining typed
    // intrinsic layouts stay in declaration order with ordinary layouts.
    let names: Vec<&str> = layout_values(&module).map(|l| l.name.as_str()).collect();
    assert_eq!(
        module.meta.layouts[module.meta.well_known_layouts.string].kind,
        lir::LayoutKind::Intrinsic(lir::IntrinsicTypeRepresentation::String)
    );
    assert_eq!(
        names,
        [
            "S",
            "Outer",
            "Int",
            "UInt",
            "Boolean",
            "String",
            "(String, Int)",
            "(Boolean, Int)"
        ]
    );

    let string = &module.meta.layouts[module.meta.well_known_layouts.string];
    assert_eq!((string.size, string.align), (24, 8));
    assert!(string.fields.is_empty());

    // S { a: Int @0, s: String @8 }: size 16, align 8, refs [8].
    let s_layout = by_name("S");
    assert_eq!((s_layout.size, s_layout.align), (16, 8));
    assert_eq!(plain_refs(s_layout), [8]);

    // Outer { flag: Boolean @0, pair: (String, Int) @8 } with the
    // String at pair+0: size 24, align 8, refs [8].
    let outer = by_name("Outer");
    assert_eq!((outer.size, outer.align), (24, 8));
    assert_eq!(plain_refs(outer), [8]);

    // The tuple field type gets its own layout too.
    let pair_layout = by_name("(String, Int)");
    assert_eq!((pair_layout.size, pair_layout.align), (16, 8));
    assert_eq!(plain_refs(pair_layout), [0]);

    // (Boolean, Int): Int is 8-aligned, so it sits at offset 8 and
    // the size rounds up to 16.
    let padded = by_name("(Boolean, Int)");
    assert_eq!((padded.size, padded.align), (16, 8));
    assert!(plain_refs(padded).is_empty());
}
