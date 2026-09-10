use super::super::*;

#[test]
fn params_and_return_translate() {
    let mut h = Harness::new();
    let int = h.int;
    let mut locals = Arena::new();
    let x = locals.alloc(local("x", int));
    let y = locals.alloc(local("y", int));
    // fun add(x: Int, y: Int): Int { return x + y }
    let sum = integer_binary(
        &mut h,
        hir::IntegerKind::SIGNED_32,
        hir::NoGcIntegerOperation::Add,
        local_ref(x, int),
        local_ref(y, int),
    );
    let add = h.user_fn_full(
        "add",
        Vec::new(),
        vec![param("x", int, x), param("y", int, y)],
        int,
        hir::Body {
            locals,
            statements: vec![stmt(hir::StatementKind::Return { value: Some(sum) })],
        },
    );
    let main = h.user_fn(
        "main",
        hir::Body {
            locals: Arena::new(),
            statements: vec![expr_stmt(call(
                &h,
                add,
                vec![int_lit(&h, 1), int_lit(&h, 2)],
            ))],
        },
    );
    let module = lower(&h.finish(main));

    let add_fn = &module.functions[module.top_level[0]];
    assert_eq!(add_fn.symbol, "scoop.add");
    assert_eq!(add_fn.params.len(), 2);
    let int_ty = mir::Type::Integer(mir::IntegerKind::SIGNED_32);
    assert_eq!(add_fn.params[0].ty, int_ty);
    assert_eq!(add_fn.params[1].ty, int_ty);
    assert_eq!(add_fn.return_ty, int_ty);
    // Parameters are (the first) locals of the body.
    let px = add_fn.params[0].local;
    assert_eq!(add_fn.body.locals[px].name, "x");
    assert!(matches!(
        &add_fn.body.blocks[add_fn.body.entry].terminator,
        mir::Terminator::Return {
            value: Some(value)
        } if matches!(value.kind, mir::ExprKind::IntegerBinary {
            operation,
            ..
        } if operation == mir::IntegerBinaryOperation::new(
            mir::IntegerKind::SIGNED_32,
            mir::IntegerBinaryOperator::Add,
        ))
    ));
}

#[test]
fn monomorphizes_generic_functions() {
    let mut h = Harness::new();
    let identity = identity_fn(&mut h, "identity");
    let (int, string) = (h.int, h.string);
    let identity_int = h.instantiate(identity, vec![int]);
    let identity_string = h.instantiate(identity, vec![string]);
    let main = h.user_fn(
        "main",
        hir::Body {
            locals: Arena::new(),
            statements: vec![
                expr_stmt(generic_call(identity_int, vec![int_lit(&h, 41)], int)),
                expr_stmt(generic_call(
                    identity_string,
                    vec![str_lit(&h, "hi")],
                    string,
                )),
            ],
        },
    );
    let module = lower(&h.finish(main));

    // main first (declaration order), then the instances in
    // creation order. The generic function itself has no MIR body.
    assert_eq!(module.top_level.len(), 3);
    let int_instance = &module.functions[module.top_level[1]];
    let string_instance = &module.functions[module.top_level[2]];
    assert_eq!(int_instance.symbol, "scoop.identity$I32");
    assert_eq!(string_instance.symbol, "scoop.identity$S");

    // The instance signature, locals and body are fully
    // substituted — no `Param` survives.
    assert_eq!(int_instance.params.len(), 1);
    let int_ty = mir::Type::Integer(mir::IntegerKind::SIGNED_32);
    assert_eq!(int_instance.params[0].ty, int_ty);
    assert_eq!(int_instance.return_ty, int_ty);
    let x = int_instance.params[0].local;
    assert_eq!(int_instance.body.locals[x].ty, int_ty);
    assert!(matches!(
        &int_instance.body.blocks[int_instance.body.entry].terminator,
        mir::Terminator::Return {
            value: Some(value)
        } if matches!(value.kind, mir::ExprKind::Local(local) if local == x)
    ));
    assert_eq!(string_instance.params[0].ty, mir::Type::String);
    assert_eq!(string_instance.return_ty, mir::Type::String);

    let int_value = module
        .meta
        .source_local_values
        .get(module.top_level[1], int_instance.params[0].local)
        .expect("the Int parameter keeps its LocalConcrete value identity");
    let string_value = module
        .meta
        .source_local_values
        .get(module.top_level[2], string_instance.params[0].local)
        .expect("the String parameter keeps its LocalConcrete value identity");
    assert_ne!(
        int_value.identity_record().id(),
        string_value.identity_record().id(),
        "the same template parameter has a distinct value in each materialization"
    );

    // MIR gives every materialized body its own typed identity and
    // records symbol -> generic source provenance in the meta.
    assert_eq!(module.meta.instances.len(), 2);
    let int_meta = &module.meta.instances[instance_id(&module, module.top_level[1])];
    assert_eq!(int_meta.symbol, "scoop.identity$I32");
    assert_eq!(int_meta.display_name, "identity");
    assert_eq!(
        int_value.identity_record().key().owner(),
        int_meta.materialization
    );
    assert!(matches!(
        int_meta.materialization.template(),
        scoop_identity::CallableTemplateOwner::GenericFunction(_)
    ));
    let scoop_identity::CallableMaterializationContext::Application(int_application) =
        int_meta.materialization.context()
    else {
        panic!("identity<Int> must retain its persistent callable application")
    };
    let string_meta = &module.meta.instances[instance_id(&module, module.top_level[2])];
    assert_eq!(
        string_meta.materialization.template(),
        int_meta.materialization.template()
    );
    let scoop_identity::CallableMaterializationContext::Application(string_application) =
        string_meta.materialization.context()
    else {
        panic!("identity<String> must retain its persistent callable application")
    };
    assert_ne!(int_application, string_application);

    // The calls in main resolve to the two instances.
    let main_fn = &module.functions[module.entry];
    for (statement, instance) in entry_statements(&main_fn.body)
        .iter()
        .zip([module.top_level[1], module.top_level[2]])
    {
        let (call, _) = statement_call(statement);
        assert_eq!(
            call.target.callee,
            mir::Callee::Monomorphized(instance_id(&module, instance))
        );
    }
}

#[test]
fn duplicate_requests_produce_one_instance() {
    let mut h = Harness::new();
    let identity = identity_fn(&mut h, "identity");
    let int = h.int;
    let identity_int = h.instantiate(identity, vec![int]);
    let main = h.user_fn(
        "main",
        hir::Body {
            locals: Arena::new(),
            statements: vec![
                expr_stmt(generic_call(identity_int, vec![int_lit(&h, 1)], int)),
                expr_stmt(generic_call(identity_int, vec![int_lit(&h, 2)], int)),
            ],
        },
    );
    // HIR dedups its list, but be robust: the same request listed
    // twice, plus two calls with the same type arguments.
    assert_eq!(h.instantiate(identity, vec![int]), identity_int);
    let module = lower(&h.finish(main));

    assert_eq!(module.top_level.len(), 2);
    let instance = module.top_level[1];
    let main_fn = &module.functions[module.entry];
    for statement in entry_statements(&main_fn.body) {
        let (call, _) = statement_call(statement);
        assert_eq!(
            call.target.callee,
            mir::Callee::Monomorphized(instance_id(&module, instance))
        );
    }
}

#[test]
fn nested_generic_calls_extend_the_worklist() {
    let mut h = Harness::new();
    // fun <T> inner(x: T): T { return x }
    let inner = identity_fn(&mut h, "inner");
    // fun <T> forward(x: T): T { return inner(x) }
    let t = h
        .types
        .alloc(hir::Type::Param(hir::TypeParamId::from_raw(0)));
    let mut locals = Arena::new();
    let x = locals.alloc(local("x", t));
    let inner_t = h.instantiate(inner, vec![t]);
    let forward = h.user_fn_full(
        "forward",
        vec!["T".to_string()],
        vec![param("x", t, x)],
        t,
        hir::Body {
            locals,
            statements: vec![stmt(hir::StatementKind::Return {
                value: Some(generic_call(inner_t, vec![local_ref(x, t)], t)),
            })],
        },
    );
    let int = h.int;
    let forward_int = h.instantiate(forward, vec![int]);
    let main = h.user_fn(
        "main",
        hir::Body {
            locals: Arena::new(),
            statements: vec![expr_stmt(generic_call(
                forward_int,
                vec![int_lit(&h, 1)],
                int,
            ))],
        },
    );
    // The nested request is parameterized in export HIR's list;
    // local-concrete HIR resolves it while materializing forward$I32.
    let module = lower(&h.finish(main));

    // main, forward$I32, then inner$I32 (discovered via the worklist).
    assert_eq!(module.top_level.len(), 3);
    let forward_i = &module.functions[module.top_level[1]];
    let inner_i = &module.functions[module.top_level[2]];
    assert_eq!(forward_i.symbol, "scoop.forward$I32");
    assert_eq!(inner_i.symbol, "scoop.inner$I32");
    let (call, destination) = statement_call(&entry_statements(&forward_i.body)[0]);
    let destination = destination.expect("inner$I32 returns Int");
    assert_eq!(
        call.target.callee,
        mir::Callee::Monomorphized(instance_id(&module, module.top_level[2]))
    );
    assert!(matches!(
        &forward_i.body.blocks[forward_i.body.entry].terminator,
        mir::Terminator::Return {
            value: Some(value)
        } if matches!(value.kind, mir::ExprKind::Local(local) if local == destination)
    ));
    assert_eq!(
        inner_i.params[0].ty,
        mir::Type::Integer(mir::IntegerKind::SIGNED_32)
    );
    assert_eq!(
        inner_i.return_ty,
        mir::Type::Integer(mir::IntegerKind::SIGNED_32)
    );
}

#[test]
fn instance_symbols_encode_enum_and_tuple_arguments() {
    let mut h = Harness::new();
    let f = identity_fn(&mut h, "f");
    let (int, string) = (h.int, h.string);
    let option_int = h.option(int);
    let pair = h.tuple(&[int, string]);
    let main = h.user_fn(
        "main",
        hir::Body {
            locals: Arena::new(),
            statements: Vec::new(),
        },
    );
    h.instantiate(f, vec![option_int]);
    h.instantiate(f, vec![pair]);
    let module = lower(&h.finish(main));

    let symbols: Vec<&str> = module.top_level[1..]
        .iter()
        .map(|&id| module.functions[id].symbol.as_str())
        .collect();
    // An enum argument encodes the category, length-delimited
    // instance name, and complete argument list (`mir::encode_type`).
    assert_eq!(symbols, ["scoop.f$E6_OptionAI32X", "scoop.f$TI32_SX"]);
    // Substitution recurses into enum / tuple types.
    let option_instance = &module.functions[module.top_level[1]];
    let mir::Type::Enum(enum_id, args) = &option_instance.params[0].ty else {
        panic!("the Option<Int> instance parameter must be an enum type")
    };
    assert_eq!(module.enums[*enum_id].name, "Option$I32");
    assert_eq!(
        args.as_slice(),
        &[mir::Type::Integer(mir::IntegerKind::SIGNED_32)]
    );
    let tuple_instance = &module.functions[module.top_level[2]];
    assert_eq!(
        tuple_instance.return_ty,
        mir::Type::Tuple(vec![
            mir::Type::Integer(mir::IntegerKind::SIGNED_32),
            mir::Type::String,
        ])
    );
}
