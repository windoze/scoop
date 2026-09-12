use super::*;

fn string_identity(
    owner: scoop_identity::CallableMaterialization,
    ordinal: u32,
) -> mir::ImmortalObjectKey {
    mir::ImmortalObjectKey::string_constant(
        mir::ImmortalObjectOwner::Callable(owner),
        mir::StructuralDefinitionPath::from_first(
            mir::StructuralPathSegment::new(
                mir::StructuralDefinitionSiteRole::StringConstant,
                ordinal,
            ),
            [],
        ),
    )
}

#[test]
fn lowers_hello_world() {
    let source = hello_world();
    let concrete = scoop_hir_lower::concretize_legacy_export(&source);
    let expected_string_identities = [
        string_identity(concrete.functions[concrete.top_level[1]].materialization, 0),
        string_identity(concrete.functions[concrete.top_level[2]].materialization, 0),
        string_identity(concrete.functions[concrete.entry()].materialization, 0),
    ];
    let module = lower(&source);

    // Intrinsics are excluded from `top_level`; declaration order
    // kept: the two core overloads the test uses, then the user
    // functions.
    assert_eq!(module.top_level.len(), 4);
    let helper = &module.functions[module.top_level[2]];
    let main = &module.functions[module.top_level[3]];
    assert_eq!(helper.name, "helper");
    assert_eq!(main.name, "main");

    assert_eq!(module.entry, module.top_level[3]);
    assert!(
        module
            .meta
            .source_callable_materializations
            .get(module.top_level[2])
            .is_some()
    );
    assert!(
        module
            .meta
            .source_callable_materializations
            .get(module.entry)
            .is_some()
    );

    // String literals became numbered global constants (in lowering
    // order: function bodies are lowered in declaration order, so
    // core's `println(String)` contributes its `"\n"` first).
    let strings: Vec<&str> = module
        .strings
        .iter()
        .map(|(_, s)| s.value.as_str())
        .collect();
    assert_eq!(strings, ["\n", "!", "hello, world"]);
    assert_eq!(
        module
            .strings
            .iter()
            .map(|(_, string)| &string.identity)
            .collect::<Vec<_>>(),
        expected_string_identities.iter().collect::<Vec<_>>()
    );

    // M2 meta exists but is empty.
    assert!(module.meta.dispatch_tables.is_empty());

    // Golden dump locks the output structure.
    let expected = "\
Module
  extern ef0 write @scoop_rt_write(String) -> Unit <abi=scoop managed>
  fun print @fn0(message: String) -> Unit
    bb0 entry
      call extern0 @scoop_rt_write direct
        Type String
        Local message
      return
  fun println @fn1(message: String) -> Unit
    bb0 entry
      call extern0 @scoop_rt_write direct
        Type String
        Local message
      call extern0 @scoop_rt_write direct
        Type String
        StringConst @str0
      return
  fun helper @fn2() -> Unit
    bb0 entry
      call @fn0 direct
        Type String
        StringConst @str1
      return
  fun main @fn3() -> Unit
    bb0 entry
      call @fn1 direct
        Type String
        StringConst @str2
      call @fn2 direct
      return
  str @str0 \"\\n\"
  str @str1 \"!\"
  str @str2 \"hello, world\"
  entry @fn3
";
    assert_eq!(dump(&module), expected);
}

#[test]
fn repeated_literals_get_separate_constants_deterministically() {
    let executable = hello_world();
    // Add another `println("hello, world")` to `main`. The
    // `println(String)` overload is the second function in
    // `top_level` (after `print(String)`).
    let println = executable.top_level[1];
    let string = executable.string;
    let unit = executable.unit;
    let main_id = executable.entry();
    let mut hir_module = executable.into_module();
    let hir::FunctionKind::User(body) = &mut hir_module.functions[main_id].kind else {
        unreachable!()
    };
    body.statements.push(hir::Statement {
        kind: hir::StatementKind::Expr(hir::Expr {
            kind: hir::ExprKind::Call {
                callee: hir::Callable::Function(println),
                args: vec![hir::Expr {
                    kind: hir::ExprKind::StringLiteral {
                        value: "hello, world".to_string(),
                        owner: hir::StringConstantOwner::CurrentDefinition,
                    },
                    ty: string,
                    span: SPAN,
                    origin: expression_origin(),
                }],
            },
            ty: unit,
            span: SPAN,
            origin: expression_origin(),
        }),
        span: SPAN,
    });

    let hir_module = legacy_executable(hir_module, main_id);
    let module = lower(&hir_module);
    let values: Vec<&str> = module
        .strings
        .iter()
        .map(|(_, s)| s.value.as_str())
        .collect();
    assert_eq!(values, ["\n", "!", "hello, world", "hello, world"]);
}

#[test]
fn formatting_helpers_are_ordinary_extern_calls() {
    let mut h = Harness::new();
    let long_to_string = h.long_to_string();
    let bool_to_string = h.bool_to_string();
    let main = h.user_fn(
        "main",
        hir::Body {
            locals: Arena::new(),
            statements: vec![
                expr_stmt(call_typed(
                    long_to_string,
                    vec![integer_lit(&h, hir::IntegerKind::SIGNED_64, 1)],
                    h.string,
                )),
                expr_stmt(call_typed(
                    bool_to_string,
                    vec![bool_lit(&h, true)],
                    h.string,
                )),
            ],
        },
    );
    let module = lower(&h.finish(main));

    let body = &module.functions[module.entry].body;
    let symbols: Vec<&str> = entry_statements(body)
        .iter()
        .map(|statement| {
            let (call, _) = statement_call(statement);
            let mir::Callee::Extern(function) = call.target.callee else {
                panic!("formatting helpers must remain ordinary extern callees")
            };
            module.extern_functions[function].native_symbol.as_str()
        })
        .collect();
    assert_eq!(
        symbols,
        ["scoop_rt_long_to_string", "scoop_rt_bool_to_string"]
    );
}

#[test]
fn raw_struct_construction_reaches_mir_without_a_constructor_call() {
    let mut h = Harness::new();
    let point = h.strukt("Point", &[("x", h.int), ("y", h.int)]);
    let point_ty = h.struct_ty(point);
    let application = h.struct_application_of(point_ty);
    let mut locals = Arena::new();
    let value = locals.alloc(local("value", point_ty));
    let main = h.user_fn(
        "main",
        hir::Body {
            locals,
            statements: vec![val_decl(
                value,
                expr(
                    hir::ExprKind::StructConstruct {
                        application,
                        fields: vec![int_lit(&h, 20), int_lit(&h, 10)],
                    },
                    point_ty,
                ),
            )],
        },
    );

    let module = lower(&h.finish(main));
    assert_eq!(module.validate(), Ok(()));
    let statements = entry_statements(&module.functions[module.entry].body);
    assert!(matches!(
        statements,
        [mir::Statement {
            kind: mir::StatementKind::ValDecl {
                init: mir::Expr {
                    kind: mir::ExprKind::StructConstruct { struct_id, fields },
                    ..
                },
                ..
            },
            ..
        }] if *struct_id == mir::StructId::from_raw(point.into_raw()) && fields.len() == 2
    ));
    assert!(
        statements
            .iter()
            .all(|statement| !matches!(statement.kind, mir::StatementKind::Call(_))),
        "raw reconstruction must not call a source constructor"
    );
    let dump = dump(&module);
    assert!(dump.contains("StructConstruct Point"), "{dump}");
}

// ---- M9: GC intrinsics and generic structs ----

/// The source-level bodies of `pin` / `unpin` / handle operations after
/// inlining their ordinary wrappers: raw runtime call plus explicit
/// handle construction or field extraction.
fn gc_shapes() -> (Harness, hir::FunctionId) {
    let mut h = Harness::new();
    let gc = h.gc_core();
    let (string, ulong) = (h.string, h.ulong());
    let pinned_ptr_s = h.struct_app(gc.pinned_ptr, vec![string]);
    let gc_handle_s = h.struct_app(gc.gc_handle, vec![string]);
    let pinned_ptr_s_application = h.struct_application_of(pinned_ptr_s);
    let gc_handle_s_application = h.struct_application_of(gc_handle_s);
    let pin_raw = h.instantiate(gc.pin_raw, vec![string]);
    let unpin_raw = h.instantiate(gc.unpin_raw, vec![string]);
    let get_handle_raw = h.instantiate(gc.get_handle_raw, vec![string]);
    let release_handle_raw = h.instantiate(gc.release_handle_raw, vec![string]);
    let mut locals = Arena::new();
    let s = locals.alloc(local("s", string));
    let pin_word = locals.alloc(local("pinWord", ulong));
    let ph = locals.alloc(local("ph", pinned_ptr_s));
    let r = locals.alloc(local("r", string));
    let handle_word = locals.alloc(local("handleWord", ulong));
    let gh = locals.alloc(local("gh", gc_handle_s));
    let r2 = locals.alloc(local("r2", string));
    let n = locals.alloc(local("n", ulong));
    let main = h.user_fn(
        "main",
        hir::Body {
            locals,
            statements: vec![
                val_decl(
                    pin_word,
                    generic_call(pin_raw, vec![local_ref(s, string)], ulong),
                ),
                val_decl(
                    ph,
                    struct_init(&h, pinned_ptr_s, vec![local_ref(pin_word, ulong)]),
                ),
                val_decl(
                    r,
                    generic_call(
                        unpin_raw,
                        vec![expr(
                            hir::ExprKind::FieldAccess {
                                receiver: Box::new(local_ref(ph, pinned_ptr_s)),
                                field: h.struct_field_ref(pinned_ptr_s_application, 0),
                            },
                            ulong,
                        )],
                        string,
                    ),
                ),
                val_decl(
                    handle_word,
                    generic_call(get_handle_raw, vec![local_ref(s, string)], ulong),
                ),
                val_decl(
                    gh,
                    struct_init(&h, gc_handle_s, vec![local_ref(handle_word, ulong)]),
                ),
                val_decl(
                    r2,
                    generic_call(
                        release_handle_raw,
                        vec![expr(
                            hir::ExprKind::FieldAccess {
                                receiver: Box::new(local_ref(gh, gc_handle_s)),
                                field: h.struct_field_ref(gc_handle_s_application, 0),
                            },
                            ulong,
                        )],
                        string,
                    ),
                ),
                expr_stmt(call(&h, gc.gc_collect, vec![])),
                val_decl(n, call_typed(gc.gc_stats, vec![], ulong)),
            ],
        },
    );
    (h, main)
}

#[test]
fn gc_wrappers_use_explicit_raw_word_marshalling() {
    let (h, main) = gc_shapes();
    let module = lower(&h.finish(main));
    let body = &module.functions[module.entry].body;

    // `_pin(s)` produces the raw word used by `PinnedPtr(raw)`.
    let (call, pin_result) = statement_call(&entry_statements(body)[0]);
    assert!(matches!(
        call.target.callee,
        mir::Callee::Runtime(mir::RuntimeFn::Pin)
    ));
    assert!(matches!(call.args.as_slice(), [arg] if matches!(arg.kind, mir::ExprKind::Local(_))));
    let pin_result = pin_result.expect("_pin returns a raw word");
    let (constructor, wrapped) = statement_call(&entry_statements(body)[1]);
    let wrapped = wrapped.expect("PinnedPtr constructor returns the wrapper");
    assert!(matches!(constructor.target.callee, mir::Callee::User(_)));
    assert!(matches!(constructor.args.as_slice(), [arg]
            if matches!(arg.kind, mir::ExprKind::Local(local) if local == pin_result)));
    assert_eq!(body.locals[wrapped].name, "ph");

    // `_unpin(ph.raw)` directly initializes the source result local.
    let (call, hidden) = statement_call(&entry_statements(body)[2]);
    let hidden = hidden.expect("_unpin produces the typed result");
    assert!(matches!(
        call.target.callee,
        mir::Callee::Runtime(mir::RuntimeFn::Unpin)
    ));
    let [arg] = call.args.as_slice() else {
        panic!("unpin takes one argument")
    };
    let mir::ExprKind::FieldAccess { index: 0, .. } = arg.kind else {
        panic!("unpin's argument is the handle's raw field")
    };
    assert_eq!(body.locals[hidden].name, "r");

    // `getGcHandle` / `releaseGcHandle` share the wrap / unwrap
    // shapes with their own runtime symbols and handle struct.
    let (call, handle_result) = statement_call(&entry_statements(body)[3]);
    assert!(matches!(
        call.target.callee,
        mir::Callee::Runtime(mir::RuntimeFn::GetHandle)
    ));
    let handle_result = handle_result.expect("getGcHandle returns a raw word");
    let (constructor, wrapped) = statement_call(&entry_statements(body)[4]);
    let wrapped = wrapped.expect("GcHandle constructor returns the wrapper");
    assert!(matches!(constructor.target.callee, mir::Callee::User(_)));
    assert!(matches!(constructor.args.as_slice(), [arg]
            if matches!(arg.kind, mir::ExprKind::Local(local) if local == handle_result)));
    assert_eq!(body.locals[wrapped].name, "gh");
    let (call, _) = statement_call(&entry_statements(body)[5]);
    assert!(matches!(
        call.target.callee,
        mir::Callee::Runtime(mir::RuntimeFn::ReleaseHandle)
    ));
    let [arg] = call.args.as_slice() else {
        panic!("releaseGcHandle takes one argument")
    };
    let mir::ExprKind::FieldAccess { index: 0, .. } = arg.kind else {
        panic!("releaseGcHandle's argument is the handle's raw field")
    };

    // `gcCollect()` is a plain void runtime call; `gcStats()`
    // yields the raw word (`UInt`).
    let (call, destination) = statement_call(&entry_statements(body)[6]);
    assert!(destination.is_none());
    assert!(matches!(
        call.target.callee,
        mir::Callee::Runtime(mir::RuntimeFn::GcCollect)
    ));
    let (call, destination) = statement_call(&entry_statements(body)[7]);
    assert!(matches!(
        call.target.callee,
        mir::Callee::Runtime(mir::RuntimeFn::GcStats)
    ));
    assert_eq!(
        destination,
        Some(
            *body
                .locals
                .iter()
                .find(|(_, local)| local.name == "n")
                .map(|(id, _)| id)
                .as_ref()
                .expect("n local")
        )
    );
}
