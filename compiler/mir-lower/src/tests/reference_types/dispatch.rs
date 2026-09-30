use super::*;

#[test]
fn method_calls_are_annotated_by_the_receiver_static_type() {
    let mut h = Harness::new();
    let iface = h.interface("Describable", &["describe", "label"]);
    let iface_ty = h.interface_ty(iface);
    let class = h.class("C", hir::ClassModifier::Open, &[], None, &[iface]);
    let class_ty = h.class_ty(class);
    let class_describe = empty_method(&mut h, "C", "describe", class_ty);
    let _class_label = empty_method(&mut h, "C", "label", class_ty);
    let iface_label = h.interface_methods[h.interfaces[iface].methods[1]].function;
    // A value type method.
    let int = h.int;
    let s = h.strukt("S", &[("x", int)]);
    let s_ty = h.struct_ty(s);
    let s_describe = empty_method(&mut h, "S", "describe", s_ty);
    let class_describe = h.method_application(class_describe);
    let iface_label = h.method_application(iface_label);
    let s_describe = h.method_application(s_describe);

    let unit = h.unit;
    let mut locals = Arena::new();
    let c = locals.alloc(local("c", class_ty));
    let i = locals.alloc(local("i", iface_ty));
    let sv = locals.alloc(local("sv", s_ty));
    let method_call = |receiver: hir::Expr, application: hir::MethodApplicationId| {
        expr(
            hir::ExprKind::MethodCall {
                receiver: Box::new(receiver),
                callee: hir::MethodCallee::Callable(hir::Callable::Method(application).into()),
                args: Vec::new(),
            },
            unit,
        )
    };
    let main = h.user_fn(
        "main",
        hir::Body {
            locals,
            statements: vec![
                expr_stmt(method_call(local_ref(c, class_ty), class_describe)),
                expr_stmt(method_call(local_ref(i, iface_ty), iface_label)),
                expr_stmt(method_call(local_ref(sv, s_ty), s_describe)),
            ],
        },
    );
    let module = lower(&h.finish(main));

    let body = &module.functions[module
        .output
        .executable_entry()
        .expect("test module is executable")]
    .body;
    let call_kind = |index: usize| {
        let (call, _) = statement_call(&entry_statements(body)[index]);
        // The receiver becomes argument 0 (`this`).
        assert!(!call.args.is_empty());
        &call.target.kind
    };
    // Class receiver: virtual through its ordinary vtable.
    assert!(matches!(call_kind(0), mir::CallKind::Virtual { slot: 0 }));
    // Interface receiver: the method's declaration index is the
    // itable slot.
    assert!(matches!(
        call_kind(1),
        mir::CallKind::Interface { interface, slot: 1 } if *interface == la_arena::Idx::from_raw(0.into())
    ));
    // Value type receiver: direct.
    assert!(matches!(call_kind(2), mir::CallKind::Direct));

    let (method_id, method) = module
        .functions
        .iter()
        .find(|(_, function)| function.name == "C.describe")
        .expect("the class method is lowered");
    let source = module
        .meta
        .source_callable_materializations
        .get(method_id)
        .expect("the class method has one source callable identity");
    let receiver = module
        .meta
        .source_exact_types
        .get(&method.params[0].ty)
        .expect("the concrete class receiver has one exact identity")
        .identity_record()
        .id();
    let signature = source.signature_record().signature();
    assert_eq!(
        signature.receiver(),
        scoop_identity::OptionalExactOwner::Present(receiver)
    );
    assert!(
        signature.parameters().is_empty(),
        "the physical receiver must not be duplicated in logical parameters"
    );
}

#[test]
fn final_methods_are_direct_while_final_overrides_keep_the_base_slot() {
    let mut h = Harness::new();
    let base = h.class("Base", hir::ClassModifier::Open, &[], None, &[]);
    let base_ty = h.class_ty(base);
    let base_open = empty_method(&mut h, "Base", "openMethod", base_ty);
    let base_final = empty_method(&mut h, "Base", "finalMethod", base_ty);
    h.functions[base_final]
        .method
        .as_mut()
        .expect("method")
        .modifier = hir::MethodModifier::Final;
    h.functions[base_final]
        .method
        .as_mut()
        .expect("method")
        .dispatch = hir::MethodDispatch::Direct;

    let derived = h.class(
        "Derived",
        hir::ClassModifier::Final,
        &[],
        Some((base, vec![])),
        &[],
    );
    let derived_ty = h.class_ty(derived);
    let derived_override = empty_method(&mut h, "Derived", "openMethod", derived_ty);
    h.functions[derived_override]
        .method
        .as_mut()
        .expect("method")
        .modifier = hir::MethodModifier::Final;
    let hir::MethodDispatch::Virtual(family) = h.functions[derived_override]
        .method
        .expect("method")
        .dispatch
    else {
        panic!("the test override inherits a virtual family")
    };
    h.functions[derived_override]
        .method
        .as_mut()
        .expect("method")
        .dispatch = hir::MethodDispatch::FinalOverride(family);
    let base_open = h.method_application(base_open);
    let base_final = h.method_application(base_final);
    let derived_override = h.method_application(derived_override);

    let unit = h.unit;
    let mut locals = Arena::new();
    let as_base = locals.alloc(local("asBase", base_ty));
    let as_derived = locals.alloc(local("asDerived", derived_ty));
    let call = |receiver: hir::Expr, application: hir::MethodApplicationId| {
        expr(
            hir::ExprKind::MethodCall {
                receiver: Box::new(receiver),
                callee: hir::MethodCallee::Callable(hir::Callable::Method(application).into()),
                args: Vec::new(),
            },
            unit,
        )
    };
    let main = h.user_fn(
        "main",
        hir::Body {
            locals,
            statements: vec![
                expr_stmt(call(local_ref(as_base, base_ty), base_open)),
                expr_stmt(call(local_ref(as_base, base_ty), base_final)),
                expr_stmt(call(local_ref(as_derived, derived_ty), derived_override)),
            ],
        },
    );
    let module = lower(&h.finish(main));

    let base_vtable = &module.classes[class_index(0)].vtable;
    assert_eq!(base_vtable.len(), 1);
    assert_eq!(slot_fn(&module, &base_vtable[0]), "Base.openMethod");
    let derived_vtable = &module.classes[class_index(1)].vtable;
    assert_eq!(derived_vtable.len(), 1);
    assert_eq!(slot_fn(&module, &derived_vtable[0]), "Derived.openMethod");

    let body = &module.functions[module
        .output
        .executable_entry()
        .expect("test module is executable")]
    .body;
    let kind = |index: usize| {
        let (call, _) = statement_call(&entry_statements(body)[index]);
        &call.target.kind
    };
    assert!(matches!(kind(0), mir::CallKind::Virtual { slot: 0 }));
    assert!(matches!(kind(1), mir::CallKind::Direct));
    assert!(matches!(kind(2), mir::CallKind::Direct));
}
