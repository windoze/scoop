use super::*;

#[test]
fn boxing_only_materializes_the_payload_class() {
    let mut h = Harness::new();
    let int = h.int;
    let s = h.strukt("S", &[("x", int)]);
    let s_ty = h.struct_ty(s);
    let any = h.any();
    let mut locals = Arena::new();
    let a = locals.alloc(local("a", any));
    let main = h.user_fn(
        "main",
        hir::Body {
            locals,
            statements: vec![val_decl(
                a,
                expr(
                    hir::ExprKind::Box(Box::new(struct_init(&h, s_ty, vec![int_lit(&h, 1)]))),
                    any,
                ),
            )],
        },
    );
    let module = lower(&h.finish(main));

    let boxed = boxed_class(&module, "box$D1_SX");
    assert_eq!(boxed.declared_fields().len(), 1);
    assert_eq!(boxed.declared_fields()[0].name, "value");
    assert_eq!(
        boxed.declared_fields()[0].ty,
        mir::Type::Struct(la_arena::Idx::from_raw(0.into()))
    );
    assert!(boxed.vtable.is_empty());
    assert!(boxed.itables.is_empty());
    assert_eq!(module.meta.boxed_types.len(), 1);
    let boxed_meta = &module.meta.boxed_types[0];
    assert_eq!(
        boxed_meta.payload,
        mir::Type::Struct(la_arena::Idx::from_raw(0.into()))
    );
    assert_eq!(module.classes[boxed_meta.class].name, "box$D1_SX");
    assert!(module.functions.iter().all(|(_, function)| {
        !function.symbol.starts_with("scoop.eq.") && !function.symbol.starts_with("scoop.tostring.")
    }));
}

#[test]
fn boxed_interface_implementations_dispatch_through_adjust_thunks() {
    let mut h = Harness::new();
    let int = h.int;
    let iface = h.interface("Describable", &["describe"]);
    let iface_ty = h.interface_ty(iface);
    let s = h.strukt_with("S", &[("x", int)], &[iface]);
    let s_ty = h.struct_ty(s);
    let _describe = empty_method(&mut h, "S", "describe", s_ty);
    // `val d: Describable = S(1)` — a Box whose target is the
    // interface.
    let mut locals = Arena::new();
    let d = locals.alloc(local("d", iface_ty));
    let main = h.user_fn(
        "main",
        hir::Body {
            locals,
            statements: vec![val_decl(
                d,
                expr(
                    hir::ExprKind::Box(Box::new(struct_init(&h, s_ty, vec![int_lit(&h, 1)]))),
                    iface_ty,
                ),
            )],
        },
    );
    let module = lower(&h.finish(main));

    let boxed = boxed_class(&module, "box$D1_SX");
    assert_eq!(boxed.interfaces.len(), 1);
    assert_eq!(boxed.itables.len(), 1);
    let record = &boxed.itables[0];
    assert_eq!(record.interface, boxed.interfaces[0]);
    assert_eq!(record.slots.len(), 1);
    let thunk_symbol = slot_fn(&module, &record.slots[0]);
    assert!(thunk_symbol.starts_with("scoop.thunk."));

    // The thunk takes the boxed object as `this`, unboxes it and
    // tail-calls the value method.
    let thunk = module
        .functions
        .iter()
        .map(|(_, f)| f)
        .find(|f| f.symbol == thunk_symbol)
        .expect("the thunk is a MIR function");
    assert_eq!(thunk.params.len(), 1);
    assert_eq!(thunk.params[0].ty, mir::Type::Any);
    assert_eq!(thunk.params[0].name, "this");
    let (call, _) = statement_call(&entry_statements(&thunk.body)[0]);
    assert!(matches!(call.target.kind, mir::CallKind::Direct));
    let mir::Callee::User(impl_id) = call.target.callee else {
        panic!("the thunk calls a user function")
    };
    assert_eq!(
        module.functions[impl_id].symbol,
        symbol_of(&module, "S.describe")
    );
    assert_eq!(call.args.len(), 1);
    assert!(matches!(&call.args[0].kind, mir::ExprKind::Unbox(operand)
            if matches!(operand.kind, mir::ExprKind::Local(local) if local == thunk.params[0].local)));
}
