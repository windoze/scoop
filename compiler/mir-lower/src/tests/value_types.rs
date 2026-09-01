//! Generic functions, enums, patterns and intrinsic arrays.

use super::*;

#[test]
fn params_and_return_translate() {
    let mut h = Harness::new();
    let int = h.int;
    let mut locals = Arena::new();
    let x = locals.alloc(local("x", int));
    let y = locals.alloc(local("y", int));
    // fun add(x: Int, y: Int): Int { return x + y }
    let add = h.user_fn_full(
        "add",
        Vec::new(),
        vec![param("x", int, x), param("y", int, y)],
        int,
        hir::Body {
            locals,
            statements: vec![stmt(hir::StatementKind::Return {
                value: Some(binary(
                    hir::BinOp::Add,
                    local_ref(x, int),
                    local_ref(y, int),
                    int,
                )),
            })],
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
    assert_eq!(add_fn.params[0].ty, mir::Type::Int);
    assert_eq!(add_fn.params[1].ty, mir::Type::Int);
    assert_eq!(add_fn.return_ty, mir::Type::Int);
    // Parameters are (the first) locals of the body.
    let px = add_fn.params[0].local;
    assert_eq!(add_fn.body.locals[px].name, "x");
    assert!(matches!(
        &add_fn.body.blocks[add_fn.body.entry].terminator,
        mir::Terminator::Return {
            value: Some(value)
        } if matches!(value.kind, mir::ExprKind::Binary { op: mir::BinOp::IntAdd, .. })
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
    assert_eq!(int_instance.symbol, "scoop.identity$I");
    assert_eq!(string_instance.symbol, "scoop.identity$S");

    // The instance signature, locals and body are fully
    // substituted — no `Param` survives.
    assert_eq!(int_instance.params.len(), 1);
    assert_eq!(int_instance.params[0].ty, mir::Type::Int);
    assert_eq!(int_instance.return_ty, mir::Type::Int);
    let x = int_instance.params[0].local;
    assert_eq!(int_instance.body.locals[x].ty, mir::Type::Int);
    assert!(matches!(
        &int_instance.body.blocks[int_instance.body.entry].terminator,
        mir::Terminator::Return {
            value: Some(value)
        } if matches!(value.kind, mir::ExprKind::Local(local) if local == x)
    ));
    assert_eq!(string_instance.params[0].ty, mir::Type::String);
    assert_eq!(string_instance.return_ty, mir::Type::String);

    // MIR gives every materialized body its own typed identity and
    // records symbol -> generic source provenance in the meta.
    assert_eq!(module.meta.instances.len(), 2);
    let int_meta = &module.meta.instances[instance_id(&module, module.top_level[1])];
    assert_eq!(int_meta.symbol, "scoop.identity$I");
    assert_eq!(int_meta.source, "identity");
    assert_eq!(int_meta.type_args, vec![mir::Type::Int]);

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
    // local-concrete HIR resolves it while materializing forward$I.
    let module = lower(&h.finish(main));

    // main, forward$I, then inner$I (discovered via the worklist).
    assert_eq!(module.top_level.len(), 3);
    let forward_i = &module.functions[module.top_level[1]];
    let inner_i = &module.functions[module.top_level[2]];
    assert_eq!(forward_i.symbol, "scoop.forward$I");
    assert_eq!(inner_i.symbol, "scoop.inner$I");
    let (call, destination) = statement_call(&entry_statements(&forward_i.body)[0]);
    let destination = destination.expect("inner$I returns Int");
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
    assert_eq!(inner_i.params[0].ty, mir::Type::Int);
    assert_eq!(inner_i.return_ty, mir::Type::Int);
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
    assert_eq!(symbols, ["scoop.f$E8_Option$IAIX", "scoop.f$TI_SX"]);
    // Substitution recurses into enum / tuple types.
    let option_instance = &module.functions[module.top_level[1]];
    let mir::Type::Enum(enum_id, args) = &option_instance.params[0].ty else {
        panic!("the Option<Int> instance parameter must be an enum type")
    };
    assert_eq!(module.enums[*enum_id].name, "Option$I");
    assert_eq!(args.as_slice(), &[mir::Type::Int]);
    let tuple_instance = &module.functions[module.top_level[2]];
    assert_eq!(
        tuple_instance.return_ty,
        mir::Type::Tuple(vec![mir::Type::Int, mir::Type::String])
    );
}

#[test]
fn enum_instances_are_created_once_with_substituted_fields() {
    let mut h = Harness::new();
    let (int, string) = (h.int, h.string);
    // A non-generic enum.
    let color_variants = ["Red", "Green", "Blue"]
        .iter()
        .map(|name| hir::Variant {
            name: name.to_string(),
            fields: Vec::new(),
            defaults: Vec::new(),
        })
        .collect();
    let color = h.declare_enum("Color", Vec::new(), Vec::new(), color_variants);
    let color_ty = h.enum_ty(color);
    let s = h.strukt("S", &[("x", int)]);
    let s_ty = h.struct_ty(s);
    let option_int = h.option(int);
    let option_string = h.option(string);
    let option_s = h.option(s_ty);
    // f1 holds Option<Int> and Color; f2 holds Option<Int> again
    // (a duplicate request) and Option<String>.
    let mut locals1 = Arena::new();
    locals1.alloc(local("o", option_int));
    locals1.alloc(local("c", color_ty));
    let _f1 = h.user_fn(
        "f1",
        hir::Body {
            locals: locals1,
            statements: Vec::new(),
        },
    );
    let mut locals2 = Arena::new();
    locals2.alloc(local("o", option_int));
    locals2.alloc(local("s", option_string));
    let _f2 = h.user_fn(
        "f2",
        hir::Body {
            locals: locals2,
            statements: Vec::new(),
        },
    );
    let mut locals3 = Arena::new();
    locals3.alloc(local("s", option_s));
    let _f3 = h.user_fn(
        "f3",
        hir::Body {
            locals: locals3,
            statements: Vec::new(),
        },
    );
    let main = h.user_fn(
        "main",
        hir::Body {
            locals: Arena::new(),
            statements: Vec::new(),
        },
    );
    let module = lower(&h.finish(main));

    // One definition per (enum, type args), in creation order; the
    // duplicate Option<Int> request was deduplicated by enum identity.
    let names: Vec<&str> = module
        .enums
        .iter()
        .map(|(_, def)| def.name.as_str())
        .collect();
    assert_eq!(names, ["Option$I", "Color", "Option$S", "Option$D1_SX"]);

    // The variant field types are substituted with the instance's
    // type arguments.
    let option_int_def = &module.enums[la_arena::Idx::from_raw(0.into())];
    assert_eq!(option_int_def.variants[0].name, "Some");
    assert_eq!(option_int_def.variants[0].fields[0].ty, mir::Type::Int);
    assert!(option_int_def.gc_free);
    assert!(
        option_int_def
            .variants
            .iter()
            .all(|variant| variant.gc_free)
    );
    let option_string_def = &module.enums[la_arena::Idx::from_raw(2.into())];
    assert_eq!(
        option_string_def.variants[0].fields[0].ty,
        mir::Type::String
    );
    let option_s_def = &module.enums[la_arena::Idx::from_raw(3.into())];
    assert_eq!(
        option_s_def.variants[0].fields[0].ty,
        mir::Type::Struct(la_arena::Idx::from_raw(0.into()))
    );
    assert_ne!(option_string_def.name, option_s_def.name);
    assert!(!option_string_def.gc_free);
    assert!(!option_string_def.variants[0].gc_free);
    assert!(option_string_def.variants[1].gc_free);
    // Color's variants are all unit variants.
    let color_def = &module.enums[la_arena::Idx::from_raw(1.into())];
    assert!(color_def.gc_free);
    assert_eq!(color_def.variants.len(), 3);
    assert!(
        color_def
            .variants
            .iter()
            .all(|variant| variant.fields.is_empty() && variant.gc_free)
    );
}

#[test]
fn option_nodes_become_generic_enum_operations() {
    let mut h = Harness::new();
    let (int, boolean) = (h.int, h.boolean);
    let option_int = h.option(int);
    let mut locals = Arena::new();
    let o = locals.alloc(local("o", option_int));
    let n = locals.alloc(local("n", option_int));
    let b = locals.alloc(local("b", boolean));
    let y = locals.alloc(local("y", int));
    let main = h.user_fn(
        "main",
        hir::Body {
            locals,
            statements: vec![
                val_decl(
                    o,
                    expr(
                        hir::ExprKind::SomeWrap(Box::new(int_lit(&h, 41))),
                        option_int,
                    ),
                ),
                val_decl(n, expr(hir::ExprKind::NoneLiteral, option_int)),
                val_decl(
                    b,
                    expr(
                        hir::ExprKind::IsSome(Box::new(local_ref(o, option_int))),
                        boolean,
                    ),
                ),
                val_decl(
                    y,
                    expr(
                        hir::ExprKind::Unwrap {
                            operand: Box::new(local_ref(o, option_int)),
                            trap_on_none: false,
                        },
                        int,
                    ),
                ),
            ],
        },
    );
    let module = lower(&h.finish(main));

    let expected = "\
Module
  enum Option$I
    Some(_1: Int)
    None()
  fun main @scoop_main() -> Unit
    bb0 entry
      val o: Option$I<Int>
        Type Option$I<Int>
        VariantConstruct Option$I<Int> v0
          Type Int
          IntLiteral 41
      val n: Option$I<Int>
        Type Option$I<Int>
        VariantConstruct Option$I<Int> v1
      val b: Boolean
        Type Boolean
        Binary IntEq
          Type Int
          EnumTag
            Type Option$I<Int>
            Local o
          Type Int
          IntLiteral 0
      val y: Int
        Type Int
        EnumField v0 f0
          Type Option$I<Int>
          Local o
      return
  entry @scoop_main
";
    assert_eq!(dump(&module), expected);
}

#[test]
fn trapping_unwrap_becomes_a_guarded_extraction() {
    // val o = Some(1); val y = o!!
    let mut h = Harness::new();
    h.exception("UnwrapException");
    let int = h.int;
    let option_int = h.option(int);
    let mut locals = Arena::new();
    let o = locals.alloc(local("o", option_int));
    let y = locals.alloc(local("y", int));
    let main = h.user_fn(
        "main",
        hir::Body {
            locals,
            statements: vec![
                val_decl(
                    o,
                    expr(
                        hir::ExprKind::SomeWrap(Box::new(int_lit(&h, 1))),
                        option_int,
                    ),
                ),
                val_decl(
                    y,
                    expr(
                        hir::ExprKind::Unwrap {
                            operand: Box::new(local_ref(o, option_int)),
                            trap_on_none: true,
                        },
                        int,
                    ),
                ),
            ],
        },
    );
    let module = lower(&h.finish(main));

    // The operand is evaluated once into `$opt.1`; the tag test
    // guards the extraction, and the else branch throws
    // `UnwrapException()` (M8) — an ordinary constructor call.
    let expected = "\
Module
  enum Option$I
    Some(_1: Int)
    None()
  class UnwrapException vtable=0 itables=0
  fun main @scoop_main() -> Unit
    bb0 entry
      val o: Option$I<Int>
        Type Option$I<Int>
        VariantConstruct Option$I<Int> v0
          Type Int
          IntLiteral 1
      val $opt.1: Option$I<Int>
        Type Option$I<Int>
        Local o
      branch bb1 bb2
        Type Boolean
        Binary IntEq
          Type Int
          EnumTag
            Type Option$I<Int>
            Local $opt.1
          Type Int
          IntLiteral 0
    bb1 if.then.1
      val $uw.2: Int
        Type Int
        EnumField v0 f0
          Type Option$I<Int>
          Local $opt.1
      goto bb3
    bb2 if.else.2
      call $call.1: UnwrapException = @scoop.ctor.UnwrapException direct
      throw
        Type UnwrapException
        Local $call.1
    bb3 if.merge.3
      val y: Int
        Type Int
        Local $uw.2
      return
  fun ctor.UnwrapException @scoop.ctor.UnwrapException() -> UnwrapException
    bb0 entry
      return
        Type UnwrapException
        ClassInit UnwrapException
  entry @scoop_main
";
    assert_eq!(dump(&module), expected);
}

/// `val a: Option<Int> = None; val b = Some(1); val r = a <op> b`
/// — the shared shell of the enum equality tests.
fn when_stmt(
    subject: hir::Expr,
    arms: Vec<hir::WhenArm>,
    else_body: Option<Vec<hir::Statement>>,
) -> hir::Statement {
    stmt(hir::StatementKind::When(hir::When {
        subject,
        arms,
        else_body,
    }))
}

fn arm(pattern: hir::Pattern, guard: Option<hir::Expr>, body: Vec<hir::Statement>) -> hir::WhenArm {
    hir::WhenArm {
        pattern,
        guard,
        body,
        span: SPAN,
    }
}

#[test]
fn when_lowers_to_a_decision_sequence() {
    // val o = Some(1); when (o) { Some(x) -> print(x); None -> println("none") }
    let mut h = Harness::new();
    let print_int = h.print_int();
    let println_string = h.println_string();
    let int = h.int;
    let option_int = h.option(int);
    let option_application = h.enum_application_of(option_int);
    let mut locals = Arena::new();
    let o = locals.alloc(local("o", option_int));
    let x = locals.alloc(local("x", int));
    let main = h.user_fn(
        "main",
        hir::Body {
            locals,
            statements: vec![
                val_decl(
                    o,
                    expr(
                        hir::ExprKind::SomeWrap(Box::new(int_lit(&h, 1))),
                        option_int,
                    ),
                ),
                when_stmt(
                    local_ref(o, option_int),
                    vec![
                        arm(
                            hir::Pattern::Variant {
                                application: option_application,
                                variant: 0,
                                fields: vec![(0, hir::Pattern::Binding { local: x })],
                            },
                            None,
                            vec![expr_stmt(call(&h, print_int, vec![local_ref(x, int)]))],
                        ),
                        arm(
                            hir::Pattern::Variant {
                                application: option_application,
                                variant: 1,
                                fields: Vec::new(),
                            },
                            None,
                            vec![expr_stmt(call(
                                &h,
                                println_string,
                                vec![str_lit(&h, "none")],
                            ))],
                        ),
                    ],
                    None,
                ),
            ],
        },
    );
    let module = lower(&h.finish(main));

    // The subject is evaluated once into `$when.1`; each arm is a
    // tag comparison, then the field bindings, then the body; a
    // failed tag test falls through to the next arm. (`print` /
    // `println` are ordinary core functions — M7 — so the arms
    // call the overloads, not runtime shims.)
    let expected = "\
Module
  extern ef0 write @scoop_rt_write(String) -> Unit <abi=scoop managed>
  enum Option$I
    Some(_1: Int)
    None()
  fun print @scoop.print(message: Int) -> Unit
    bb0 entry
      call $call.1: String = @scoop_rt_int_to_string direct
        Type Int
        Local message
      call extern0 @scoop_rt_write direct
        Type String
        Local $call.1
      return
  fun println @scoop.println(message: String) -> Unit
    bb0 entry
      call extern0 @scoop_rt_write direct
        Type String
        Local message
      call extern0 @scoop_rt_write direct
        Type String
        StringConst @scoop.str.0
      return
  fun main @scoop_main() -> Unit
    bb0 entry
      val o: Option$I<Int>
        Type Option$I<Int>
        VariantConstruct Option$I<Int> v0
          Type Int
          IntLiteral 1
      val $when.1: Option$I<Int>
        Type Option$I<Int>
        Local o
      branch bb1 bb2
        Type Boolean
        Binary IntEq
          Type Int
          EnumTag
            Type Option$I<Int>
            Local $when.1
          Type Int
          IntLiteral 0
    bb1 if.then.1
      val x: Int
        Type Int
        EnumField v0 f0
          Type Option$I<Int>
          Local $when.1
      call @scoop.print direct
        Type Int
        Local x
      goto bb3
    bb2 if.else.2
      branch bb4 bb5
        Type Boolean
        Binary IntEq
          Type Int
          EnumTag
            Type Option$I<Int>
            Local $when.1
          Type Int
          IntLiteral 1
    bb3 if.merge.3
      return
    bb4 if.then.4
      call @scoop.println direct
        Type String
        StringConst @scoop.str.1
      goto bb5
    bb5 if.merge.5
      goto bb3
  str @scoop.str.0 \"\\n\"
  str @scoop.str.1 \"none\"
  entry @scoop_main
";
    assert_eq!(dump(&module), expected);
}

#[test]
fn a_failed_guard_falls_through_to_the_next_arm() {
    // when (o) { Some(x) if (x > 0) -> print(x); else -> println("neg") }
    let mut h = Harness::new();
    let print_int = h.print_int();
    let println_string = h.println_string();
    let int = h.int;
    let option_int = h.option(int);
    let option_application = h.enum_application_of(option_int);
    let mut locals = Arena::new();
    let o = locals.alloc(local("o", option_int));
    let x = locals.alloc(local("x", int));
    let else_body = || {
        vec![expr_stmt(call(
            &h,
            println_string,
            vec![str_lit(&h, "neg")],
        ))]
    };
    let main = h.user_fn(
        "main",
        hir::Body {
            locals,
            statements: vec![when_stmt(
                local_ref(o, option_int),
                vec![arm(
                    hir::Pattern::Variant {
                        application: option_application,
                        variant: 0,
                        fields: vec![(0, hir::Pattern::Binding { local: x })],
                    },
                    Some(binary(
                        hir::BinOp::Gt,
                        local_ref(x, int),
                        int_lit(&h, 0),
                        h.boolean,
                    )),
                    vec![expr_stmt(call(&h, print_int, vec![local_ref(x, int)]))],
                )],
                Some(else_body()),
            )],
        },
    );
    let module = lower(&h.finish(main));

    // The guard nests inside the tag test's then branch; failing
    // it falls through to the next arm — the `else` body here,
    // which is lowered once per fallthrough edge.
    let expected = "\
Module
  extern ef0 write @scoop_rt_write(String) -> Unit <abi=scoop managed>
  enum Option$I
    Some(_1: Int)
    None()
  fun print @scoop.print(message: Int) -> Unit
    bb0 entry
      call $call.1: String = @scoop_rt_int_to_string direct
        Type Int
        Local message
      call extern0 @scoop_rt_write direct
        Type String
        Local $call.1
      return
  fun println @scoop.println(message: String) -> Unit
    bb0 entry
      call extern0 @scoop_rt_write direct
        Type String
        Local message
      call extern0 @scoop_rt_write direct
        Type String
        StringConst @scoop.str.0
      return
  fun main @scoop_main() -> Unit
    bb0 entry
      val $when.1: Option$I<Int>
        Type Option$I<Int>
        Local o
      branch bb1 bb2
        Type Boolean
        Binary IntEq
          Type Int
          EnumTag
            Type Option$I<Int>
            Local $when.1
          Type Int
          IntLiteral 0
    bb1 if.then.1
      val x: Int
        Type Int
        EnumField v0 f0
          Type Option$I<Int>
          Local $when.1
      branch bb4 bb5
        Type Boolean
        Binary IntGt
          Type Int
          Local x
          Type Int
          IntLiteral 0
    bb2 if.else.2
      call @scoop.println direct
        Type String
        StringConst @scoop.str.2
      goto bb3
    bb3 if.merge.3
      return
    bb4 if.then.4
      call @scoop.print direct
        Type Int
        Local x
      goto bb6
    bb5 if.else.5
      call @scoop.println direct
        Type String
        StringConst @scoop.str.1
      goto bb6
    bb6 if.merge.6
      goto bb3
  str @scoop.str.0 \"\\n\"
  str @scoop.str.1 \"neg\"
  str @scoop.str.2 \"neg\"
  entry @scoop_main
";
    assert_eq!(dump(&module), expected);
}

#[test]
fn literal_patterns_match_by_equality() {
    // when (n) { 1 -> println("one"); else -> println("other") }
    let mut h = Harness::new();
    let println = h.println_string();
    let int = h.int;
    let boolean = h.boolean;
    let mut equals_locals = Arena::new();
    let left = equals_locals.alloc(local("left", int));
    let right = equals_locals.alloc(local("right", int));
    let equals = h.user_fn_full(
        "Int.equals",
        Vec::new(),
        vec![param("left", int, left), param("right", int, right)],
        boolean,
        hir::Body {
            locals: equals_locals,
            statements: vec![hir::Statement {
                kind: hir::StatementKind::Return {
                    value: Some(bool_lit(&h, true)),
                },
                span: SPAN,
            }],
        },
    );
    let mut locals = Arena::new();
    let n = locals.alloc(local("n", int));
    let main = h.user_fn(
        "main",
        hir::Body {
            locals,
            statements: vec![when_stmt(
                local_ref(n, int),
                vec![arm(
                    hir::Pattern::Literal {
                        value: int_lit(&h, 1),
                        equals: hir::Callable::Function(equals),
                        subject_ty: int,
                    },
                    None,
                    vec![expr_stmt(call(&h, println, vec![str_lit(&h, "one")]))],
                )],
                Some(vec![expr_stmt(call(
                    &h,
                    println,
                    vec![str_lit(&h, "other")],
                ))]),
            )],
        },
    );
    let module = lower(&h.finish(main));

    let body = &module.functions[module.entry].body;
    let (call, result) = entry_statements(body)
        .iter()
        .find_map(|statement| {
            matches!(statement.kind, mir::StatementKind::Call(_)).then(|| statement_call(statement))
        })
        .expect("literal pattern calls its HIR-selected equality target");
    assert!(matches!(call.target.kind, mir::CallKind::Direct));
    assert!(matches!(call.args.as_slice(), [scrutinee, literal]
            if matches!(scrutinee.kind, mir::ExprKind::Local(_))
                && matches!(literal.kind, mir::ExprKind::IntLiteral(1))));
    let result = result.expect("equals returns Boolean");
    let mir::Terminator::Branch { cond, .. } = &body.blocks[body.entry].terminator else {
        panic!("literal equality result controls the pattern branch")
    };
    assert!(matches!(cond.kind, mir::ExprKind::Local(local) if local == result));
}

#[test]
fn destructuring_val_declarations_extract_bindings() {
    // val (a, b) = (1, "x"); val Point { x, .. } = p
    let mut h = Harness::new();
    let (int, string) = (h.int, h.string);
    let point = h.strukt("Point", &[("x", int), ("y", int)]);
    let point_ty = h.struct_ty(point);
    let point_application = h.struct_application_of(point_ty);
    let pair = h.tuple(&[int, string]);
    let mut locals = Arena::new();
    let a = locals.alloc(local("a", int));
    let b = locals.alloc(local("b", string));
    let p = locals.alloc(local("p", point_ty));
    let x = locals.alloc(local("x", int));
    let main = h.user_fn(
        "main",
        hir::Body {
            locals,
            statements: vec![
                stmt(hir::StatementKind::ValDecl {
                    pattern: hir::Pattern::Tuple(vec![
                        hir::Pattern::Binding { local: a },
                        hir::Pattern::Binding { local: b },
                    ]),
                    init: expr(
                        hir::ExprKind::TupleLiteral(vec![int_lit(&h, 1), str_lit(&h, "x")]),
                        pair,
                    ),
                }),
                val_decl(
                    p,
                    struct_init(&h, point_ty, vec![int_lit(&h, 3), int_lit(&h, 4)]),
                ),
                stmt(hir::StatementKind::ValDecl {
                    pattern: hir::Pattern::Struct {
                        application: point_application,
                        fields: vec![(0, hir::Pattern::Binding { local: x })],
                    },
                    init: local_ref(p, point_ty),
                }),
            ],
        },
    );
    let module = lower(&h.finish(main));

    // Each destructuring declaration evaluates its init once into
    // a hidden local, then binds the extracted fields.
    let expected = "\
Module
  struct Point (x: Int, y: Int)
  fun main @scoop_main() -> Unit
    bb0 entry
      val $bind.1: (Int, String)
        Type (Int, String)
        TupleLiteral
          Type Int
          IntLiteral 1
          Type String
          StringConst @scoop.str.0
      val a: Int
        Type Int
        FieldAccess 0
          Type (Int, String)
          Local $bind.1
      val b: String
        Type String
        FieldAccess 1
          Type (Int, String)
          Local $bind.1
      val p: Point
        Type Point
        StructInit Point
          Type Int
          IntLiteral 3
          Type Int
          IntLiteral 4
      val $bind.2: Point
        Type Point
        Local p
      val x: Int
        Type Int
        FieldAccess 0
          Type Point
          Local $bind.2
      return
  str @scoop.str.0 \"x\"
  entry @scoop_main
";
    assert_eq!(dump(&module), expected);
}

#[test]
fn array_nodes_translate_one_to_one() {
    // val a = [1, 2, 3]; val x = a[0]; val n = a.size
    // val m: MutableArray<Int> = MutableArray(a); m[0] = 40
    let mut h = Harness::new();
    h.exception("IndexOutOfBoundsException");
    let int = h.int;
    let array_int = h.array(int);
    let mutable_int = h.mutable_array(int);
    let mut locals = Arena::new();
    let a = locals.alloc(local("a", array_int));
    let x = locals.alloc(local("x", int));
    let n = locals.alloc(local("n", int));
    let m = locals.alloc(local("m", mutable_int));
    let main = h.user_fn(
        "main",
        hir::Body {
            locals,
            statements: vec![
                val_decl(
                    a,
                    expr(
                        hir::ExprKind::ArrayLiteral(vec![
                            int_lit(&h, 1),
                            int_lit(&h, 2),
                            int_lit(&h, 3),
                        ]),
                        array_int,
                    ),
                ),
                val_decl(
                    x,
                    expr(
                        hir::ExprKind::Index {
                            receiver: Box::new(local_ref(a, array_int)),
                            index: Box::new(int_lit(&h, 0)),
                        },
                        int,
                    ),
                ),
                val_decl(
                    n,
                    expr(
                        hir::ExprKind::ArrayLen(Box::new(local_ref(a, array_int))),
                        int,
                    ),
                ),
                val_decl(
                    m,
                    expr(
                        hir::ExprKind::ArrayClone(Box::new(local_ref(a, array_int))),
                        mutable_int,
                    ),
                ),
                stmt(hir::StatementKind::Assign {
                    target: hir::AssignTarget::Index {
                        array: local_ref(m, mutable_int),
                        index: int_lit(&h, 0),
                    },
                    value: int_lit(&h, 40),
                }),
            ],
        },
    );
    let module = lower(&h.finish(main));

    // The subscript read and the indexed store both get the M8
    // bounds check: array and index evaluated once into hidden
    // locals, then `IndexOutOfBoundsException` on failure.
    let expected = "\
Module
  class IndexOutOfBoundsException vtable=0 itables=0
  fun main @scoop_main() -> Unit
    bb0 entry
      val a: Array<Int>
        Type Array<Int>
        ArrayLiteral Array$I
          Type Int
          IntLiteral 1
          Type Int
          IntLiteral 2
          Type Int
          IntLiteral 3
      val $arr.1: Array<Int>
        Type Array<Int>
        Local a
      val $idx.2: Int
        Type Int
        IntLiteral 0
      branch bb2 bb1
        Type Boolean
        Binary IntLt
          Type Int
          Local $idx.2
          Type Int
          IntLiteral 0
    bb1 logic.rhs.1
      assign $logic.1
        Type Boolean
        Binary IntGe
          Type Int
          Local $idx.2
          Type Int
          ArrayLen Array$I
            Type Array<Int>
            Local $arr.1
      goto bb3
    bb2 logic.short.2
      assign $logic.1
        Type Boolean
        BoolLiteral true
      goto bb3
    bb3 logic.merge.3
      branch bb4 bb5
        Type Boolean
        Local $logic.1
    bb4 if.then.4
      call $call.2: IndexOutOfBoundsException = @scoop.ctor.IndexOutOfBoundsException direct
      throw
        Type IndexOutOfBoundsException
        Local $call.2
    bb5 if.merge.5
      val x: Int
        Type Int
        ArrayGet Array$I
          Type Array<Int>
          Local $arr.1
          Type Int
          Local $idx.2
      val n: Int
        Type Int
        ArrayLen Array$I
          Type Array<Int>
          Local a
      val m: MutableArray<Int>
        Type MutableArray<Int>
        ArrayClone Array$I -> MutableArray$I
          Type Array<Int>
          Local a
      val $arr.3: MutableArray<Int>
        Type MutableArray<Int>
        Local m
      val $idx.4: Int
        Type Int
        IntLiteral 0
      branch bb7 bb6
        Type Boolean
        Binary IntLt
          Type Int
          Local $idx.4
          Type Int
          IntLiteral 0
    bb6 logic.rhs.6
      assign $logic.3
        Type Boolean
        Binary IntGe
          Type Int
          Local $idx.4
          Type Int
          ArrayLen MutableArray$I
            Type MutableArray<Int>
            Local $arr.3
      goto bb8
    bb7 logic.short.7
      assign $logic.3
        Type Boolean
        BoolLiteral true
      goto bb8
    bb8 logic.merge.8
      branch bb9 bb10
        Type Boolean
        Local $logic.3
    bb9 if.then.9
      call $call.4: IndexOutOfBoundsException = @scoop.ctor.IndexOutOfBoundsException direct
      throw
        Type IndexOutOfBoundsException
        Local $call.4
    bb10 if.merge.10
      array_set MutableArray$I
        Type MutableArray<Int>
        Local $arr.3
        Type Int
        Local $idx.4
        Type Int
        IntLiteral 40
      return
  fun ctor.IndexOutOfBoundsException @scoop.ctor.IndexOutOfBoundsException() -> IndexOutOfBoundsException
    bb0 entry
      return
        Type IndexOutOfBoundsException
        ClassInit IndexOutOfBoundsException
  entry @scoop_main
";
    assert_eq!(dump(&module), expected);
}

#[test]
fn instance_symbols_encode_array_arguments() {
    let mut h = Harness::new();
    let f = identity_fn(&mut h, "f");
    let int = h.int;
    let array_int = h.array(int);
    let mutable_int = h.mutable_array(int);
    let main = h.user_fn(
        "main",
        hir::Body {
            locals: Arena::new(),
            statements: Vec::new(),
        },
    );
    h.instantiate(f, vec![array_int]);
    h.instantiate(f, vec![mutable_int]);
    let module = lower(&h.finish(main));

    let symbols: Vec<&str> = module.top_level[1..]
        .iter()
        .map(|&id| module.functions[id].symbol.as_str())
        .collect();
    // `mir::encode_type`: `A<element>X` / `M<element>X`.
    assert_eq!(symbols, ["scoop.f$AIX", "scoop.f$MIX"]);
    // Substitution recurses into the array element types.
    let array_instance = &module.functions[module.top_level[1]];
    assert_eq!(
        mir::array_type(&module, &array_instance.params[0].ty),
        Some((mir::ArrayKind::Immutable, &mir::Type::Int))
    );
    let mutable_instance = &module.functions[module.top_level[2]];
    assert_eq!(
        mir::array_type(&module, &mutable_instance.return_ty),
        Some((mir::ArrayKind::Mutable, &mir::Type::Int))
    );
}
