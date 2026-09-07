use super::*;

#[test]
fn boxed_interfaces_come_from_the_declaration() {
    // `struct S(val x: Int) : Describable` boxed to `Any` — the
    // boxed itable covers the declared interface even though the
    // box target is not the interface.
    let mut h = Harness::new();
    let int = h.int;
    let iface = h.interface("Describable", &["describe"]);
    let s = h.strukt_with("S", &[("x", int)], &[iface]);
    let s_ty = h.struct_ty(s);
    let _describe = empty_method(&mut h, "S", "describe", s_ty);
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
    assert_eq!(boxed.interfaces.len(), 1);
    assert_eq!(boxed.itables.len(), 1);
    let record = &boxed.itables[0];
    assert_eq!(record.interface, boxed.interfaces[0]);
    // The slot target is the generated adjust thunk for the boxed
    // value's describe implementation.
    assert_eq!(
        slot_fn(&module, &record.slots[0]),
        "scoop.thunk.D1_SX.Describable.describe"
    );
}

#[test]
fn ref_equality_maps_to_a_primitive_pointer_comparison() {
    // `===` / `!==` are reference identity: the primitive
    // comparison on the two pointers.
    let mut h = Harness::new();
    let boolean = h.boolean;
    let c = h.class("C", hir::ClassModifier::Final, &[], None, &[]);
    let c_ty = h.class_ty(c);
    let mut locals = Arena::new();
    let x = locals.alloc(local("x", c_ty));
    let y = locals.alloc(local("y", c_ty));
    let same = locals.alloc(local("same", boolean));
    let other = locals.alloc(local("other", boolean));
    let main = h.user_fn(
        "main",
        hir::Body {
            locals,
            statements: vec![
                val_decl(
                    same,
                    binary(
                        hir::BinOp::RefEq,
                        local_ref(x, c_ty),
                        local_ref(y, c_ty),
                        boolean,
                    ),
                ),
                val_decl(
                    other,
                    binary(
                        hir::BinOp::RefNe,
                        local_ref(x, c_ty),
                        local_ref(y, c_ty),
                        boolean,
                    ),
                ),
            ],
        },
    );
    let module = lower(&h.finish(main));

    let body = &module.functions[module.entry].body;
    let op_of = |index: usize| {
        let mir::StatementKind::ValDecl { init, .. } = &entry_statements(body)[index].kind else {
            panic!("expected a val declaration")
        };
        let mir::ExprKind::Binary { op, .. } = &init.kind else {
            panic!("expected a binary expression")
        };
        *op
    };
    assert_eq!(op_of(0), mir::BinOp::RefEq);
    assert_eq!(op_of(1), mir::BinOp::RefNe);
}

#[test]
fn abstract_methods_lower_to_trap_stubs() {
    // `abstract class Base { abstract fun id(): Int }` — hir-lower
    // materializes the abstract method as a params-only bodiless
    // function (`Base.id`, no statements).
    let mut h = Harness::new();
    let int = h.int;
    let base = h.class("Base", hir::ClassModifier::Abstract, &[], None, &[]);
    let base_ty = h.class_ty(base);
    let mut locals = Arena::new();
    let this = locals.alloc(local("this", base_ty));
    let id = h.method_fn(
        "Base.id",
        base_ty,
        vec![param("this", base_ty, this)],
        int,
        hir::Body {
            locals,
            statements: Vec::new(),
        },
    );
    h.functions[id].method.as_mut().expect("a method").modifier = hir::MethodModifier::Abstract;
    let main = empty_main(&mut h);
    let module = lower(&h.finish(main));

    // The abstract method is emitted (the abstract class's vtable
    // slot references it) and traps like a pure-virtual stub.
    let base_def = &module.classes[class_index(0)];
    assert_eq!(
        slot_fn(&module, &base_def.vtable[0]),
        symbol_of(&module, "Base.id")
    );
    let stub = module
        .functions
        .iter()
        .map(|(_, f)| f)
        .find(|f| f.symbol == symbol_of(&module, "Base.id"))
        .expect("the abstract method is emitted");
    assert!(
        module
            .top_level
            .iter()
            .any(|&id| module.functions[id].symbol == symbol_of(&module, "Base.id"))
    );
    assert!(matches!(
        stub.body.blocks[stub.body.entry].terminator,
        mir::Terminator::Trap { .. }
    ));
}

#[test]
fn interface_implementations_resolve_qualified_method_names() {
    // `class Doc(val title: String) : Describable { override fun
    // describe() }` — hir-lower names the member `Doc.describe`;
    // the itable / vtable resolve it by its short name.
    let mut h = Harness::new();
    let string = h.string;
    let iface = h.interface("Describable", &["describe"]);
    let doc = h.class(
        "Doc",
        hir::ClassModifier::Final,
        &[("title", string)],
        None,
        &[iface],
    );
    let doc_ty = h.class_ty(doc);
    let _describe = empty_method(&mut h, "Doc", "describe", doc_ty);
    let main = empty_main(&mut h);
    let module = lower(&h.finish(main));

    let doc_def = &module.classes[class_index(0)];
    assert_eq!(doc_def.itables.len(), 1);
    assert_eq!(
        slot_fn(&module, &doc_def.itables[0].slots[0]),
        symbol_of(&module, "Doc.describe")
    );
    // The implementing method is a vtable method too.
    assert_eq!(
        slot_fn(&module, &doc_def.vtable[0]),
        symbol_of(&module, "Doc.describe")
    );
}
