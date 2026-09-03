use super::*;

#[test]
fn lowers_hello_world() {
    let module = lower(&hello_world());

    // Intrinsics are excluded from `top_level`; declaration order
    // kept: the two core overloads the test uses, then the user
    // functions.
    assert_eq!(module.top_level.len(), 4);
    let helper = &module.functions[module.top_level[2]];
    let main = &module.functions[module.top_level[3]];
    assert_eq!(helper.name, "helper");
    assert_eq!(main.name, "main");

    // Mangling: entry is the fixed `scoop_main`, others `scoop.<name>`.
    assert_eq!(main.symbol, mir::ENTRY_SYMBOL);
    assert_eq!(helper.symbol, "scoop.helper");
    assert_eq!(module.entry, module.top_level[3]);

    // String literals became numbered global constants (in lowering
    // order: function bodies are lowered in declaration order, so
    // core's `println(String)` contributes its `"\n"` first).
    let strings: Vec<(&str, &str)> = module
        .strings
        .iter()
        .map(|(_, s)| (s.value.as_str(), s.symbol.as_str()))
        .collect();
    assert_eq!(
        strings,
        [
            ("\n", "scoop.str.0"),
            ("!", "scoop.str.1"),
            ("hello, world", "scoop.str.2")
        ]
    );

    // M2 meta exists but is empty.
    assert!(module.meta.dispatch_tables.is_empty());

    // Golden dump locks the output structure.
    let expected = "\
Module
  extern ef0 write @scoop_rt_write(String) -> Unit <abi=scoop managed>
  fun print @scoop.print(message: String) -> Unit
    bb0 entry
      call extern0 @scoop_rt_write direct
        Type String
        Local message
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
  fun helper @scoop.helper() -> Unit
    bb0 entry
      call @scoop.print direct
        Type String
        StringConst @scoop.str.1
      return
  fun main @scoop_main() -> Unit
    bb0 entry
      call @scoop.println direct
        Type String
        StringConst @scoop.str.2
      call @scoop.helper direct
      return
  str @scoop.str.0 \"\\n\"
  str @scoop.str.1 \"!\"
  str @scoop.str.2 \"hello, world\"
  entry @scoop_main
";
    assert_eq!(dump(&module), expected);
}

#[test]
fn repeated_literals_get_separate_constants_deterministically() {
    let mut hir_module = hello_world();
    // Add another `println("hello, world")` to `main`. The
    // `println(String)` overload is the second function in
    // `top_level` (after `print(String)`).
    let println = hir_module.top_level[1];
    let string = hir_module.string;
    let unit = hir_module.unit;
    let main_id = hir_module.entry;
    let hir::FunctionKind::User(body) = &mut hir_module.functions[main_id].kind else {
        unreachable!()
    };
    body.statements.push(hir::Statement {
        kind: hir::StatementKind::Expr(hir::Expr {
            kind: hir::ExprKind::Call {
                callee: hir::Callable::Function(println),
                args: vec![hir::Expr {
                    kind: hir::ExprKind::StringLiteral("hello, world".to_string()),
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

    let module = lower(&hir_module);
    let symbols: Vec<&str> = module
        .strings
        .iter()
        .map(|(_, s)| s.symbol.as_str())
        .collect();
    assert_eq!(
        symbols,
        ["scoop.str.0", "scoop.str.1", "scoop.str.2", "scoop.str.3"]
    );
}

#[test]
fn formatting_helpers_are_ordinary_extern_calls() {
    let mut h = Harness::new();
    let int_to_string = h.int_to_string();
    let bool_to_string = h.bool_to_string();
    let main = h.user_fn(
        "main",
        hir::Body {
            locals: Arena::new(),
            statements: vec![
                expr_stmt(call_typed(int_to_string, vec![int_lit(&h, 1)], h.string)),
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
        ["scoop_rt_int_to_string", "scoop_rt_bool_to_string"]
    );
}

// ---- M9: GC intrinsics and generic structs ----

/// The source-level bodies of `pin` / `unpin` / handle operations after
/// inlining their ordinary wrappers: raw runtime call plus explicit
/// handle construction or field extraction.
fn gc_shapes() -> (Harness, hir::FunctionId) {
    let mut h = Harness::new();
    let gc = h.gc_core();
    let (string, uint) = (h.string, h.uint());
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
    let pin_word = locals.alloc(local("pinWord", uint));
    let ph = locals.alloc(local("ph", pinned_ptr_s));
    let r = locals.alloc(local("r", string));
    let handle_word = locals.alloc(local("handleWord", uint));
    let gh = locals.alloc(local("gh", gc_handle_s));
    let r2 = locals.alloc(local("r2", string));
    let n = locals.alloc(local("n", uint));
    let main = h.user_fn(
        "main",
        hir::Body {
            locals,
            statements: vec![
                val_decl(
                    pin_word,
                    generic_call(pin_raw, vec![local_ref(s, string)], uint),
                ),
                val_decl(
                    ph,
                    struct_init(&h, pinned_ptr_s, vec![local_ref(pin_word, uint)]),
                ),
                val_decl(
                    r,
                    generic_call(
                        unpin_raw,
                        vec![expr(
                            hir::ExprKind::FieldAccess {
                                receiver: Box::new(local_ref(ph, pinned_ptr_s)),
                                field: hir::FieldRef::StructField {
                                    application: pinned_ptr_s_application,
                                    index: 0,
                                },
                            },
                            uint,
                        )],
                        string,
                    ),
                ),
                val_decl(
                    handle_word,
                    generic_call(get_handle_raw, vec![local_ref(s, string)], uint),
                ),
                val_decl(
                    gh,
                    struct_init(&h, gc_handle_s, vec![local_ref(handle_word, uint)]),
                ),
                val_decl(
                    r2,
                    generic_call(
                        release_handle_raw,
                        vec![expr(
                            hir::ExprKind::FieldAccess {
                                receiver: Box::new(local_ref(gh, gc_handle_s)),
                                field: hir::FieldRef::StructField {
                                    application: gc_handle_s_application,
                                    index: 0,
                                },
                            },
                            uint,
                        )],
                        string,
                    ),
                ),
                expr_stmt(call(&h, gc.gc_collect, vec![])),
                val_decl(n, call_typed(gc.gc_stats, vec![], uint)),
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
    let mir::StatementKind::ValDecl { init, .. } = &entry_statements(body)[1].kind else {
        panic!("PinnedPtr construction is a val decl")
    };
    let mir::ExprKind::StructInit { struct_id, args } = &init.kind else {
        panic!("the raw result is wrapped into PinnedPtr")
    };
    assert_eq!(module.structs[*struct_id].name, "PinnedPtr$S");
    assert_eq!(
        module.structs[*struct_id].declared_fields()[0].ty,
        mir::Type::UInt
    );
    assert!(matches!(args.as_slice(), [arg]
            if matches!(arg.kind, mir::ExprKind::Local(local) if local == pin_result)));

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
    let mir::StatementKind::ValDecl { init, .. } = &entry_statements(body)[4].kind else {
        panic!("getGcHandle's statement is a val decl")
    };
    let mir::ExprKind::StructInit { struct_id, args } = &init.kind else {
        panic!("getGcHandle's result is wrapped into the handle struct")
    };
    assert_eq!(module.structs[*struct_id].name, "GcHandle$S");
    assert!(matches!(args.as_slice(), [arg]
            if matches!(arg.kind, mir::ExprKind::Local(local) if local == handle_result)));
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
