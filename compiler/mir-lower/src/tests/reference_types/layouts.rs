use super::*;

#[test]
fn no_gc_effect_is_preserved_in_mir() {
    let mut h = Harness::new();
    let main = empty_main(&mut h);
    h.functions[main].attributes.gc_effect = hir::GcEffect::NoGc;
    let module = lower(&h.finish(main));
    assert_eq!(
        module.functions[module.entry].gc_effect,
        mir::GcEffect::NoGc
    );
    assert!(mir::dump(&module).contains("-> Unit <no-gc>"));
}

#[test]
fn class_fields_are_base_prefix_then_own() {
    let mut h = Harness::new();
    let (int, string) = (h.int, h.string);
    let base = h.class("Base", hir::ClassModifier::Open, &[("a", int)], None, &[]);
    let derived = h.class(
        "Derived",
        hir::ClassModifier::Final,
        &[("b", string)],
        Some((base, vec![int_lit(&h, 0)])),
        &[],
    );
    let derived_ty = h.class_ty(derived);
    let derived_application = h.class_application_of(derived_ty);
    let b_field = h.classes[derived].fields[0];
    let mut locals = Arena::new();
    let d = locals.alloc(local("d", derived_ty));
    let b = locals.alloc(local("b", string));
    let main = h.user_fn(
        "main",
        hir::Body {
            locals,
            statements: vec![val_decl(
                b,
                expr(
                    hir::ExprKind::FieldAccess {
                        receiver: Box::new(local_ref(d, derived_ty)),
                        field: hir::FieldRef::ClassField {
                            application: derived_application,
                            field: b_field,
                        },
                    },
                    string,
                ),
            )],
        },
    );
    let module = lower(&h.finish(main));

    assert_eq!(visible_class_count(&module), 2);
    let base_def = &module.classes[class_index(0)];
    let derived_def = &module.classes[class_index(1)];
    let field_names = |def: &mir::ClassDef| {
        def.declared_fields()
            .iter()
            .map(|field| field.name.clone())
            .collect::<Vec<_>>()
    };
    assert_eq!(field_names(base_def), ["a"]);
    // The base prefix comes first; HIR's `ClassField` indices
    // follow the same flattened order.
    assert_eq!(field_names(derived_def), ["a", "b"]);
    assert_eq!(derived_def.declared_fields()[1].ty, mir::Type::String);
    assert_eq!(derived_def.base_class(), Some(class_index(0)));
    assert_eq!(derived_def.modifier, mir::ClassModifier::Final);
    assert_eq!(base_def.modifier, mir::ClassModifier::Open);

    // The field access keeps its 0-based index into the flattened
    // layout.
    let body = &module.functions[module.entry].body;
    let mir::StatementKind::ValDecl { init, .. } = &entry_statements(body)[0].kind else {
        panic!("expected a val declaration")
    };
    assert!(matches!(
        init.kind,
        mir::ExprKind::FieldAccess { index: 1, .. }
    ));
}

#[test]
fn vtable_layout_copies_the_base_prefix_and_replaces_overrides() {
    let mut h = Harness::new();
    let base = h.class("Base", hir::ClassModifier::Open, &[], None, &[]);
    let base_ty = h.class_ty(base);
    let _m1 = empty_method(&mut h, "Base", "m1", base_ty);
    let _m2 = empty_method(&mut h, "Base", "m2", base_ty);
    let derived = h.class(
        "Derived",
        hir::ClassModifier::Open,
        &[],
        Some((base, vec![])),
        &[],
    );
    let derived_ty = h.class_ty(derived);
    // `m2` overrides the base method (same slot), `m3` is new
    // (appended after the base's slots).
    let _m2_derived = empty_method(&mut h, "Derived", "m2", derived_ty);
    let _m3 = empty_method(&mut h, "Derived", "m3", derived_ty);
    let main = empty_main(&mut h);
    let module = lower(&h.finish(main));

    let vtable_symbols = |def: &mir::ClassDef| {
        def.vtable
            .iter()
            .map(|slot| slot_fn(&module, slot))
            .collect::<Vec<_>>()
    };
    // Ordinary member functions are the whole vtable; Any does not
    // reserve compiler-owned slots.
    assert_eq!(
        vtable_symbols(&module.classes[class_index(0)]),
        [symbol_of(&module, "Base.m1"), symbol_of(&module, "Base.m2")]
    );
    // The base prefix is preserved; the override replaces slot 1
    // in place; the new method appends at slot 2.
    assert_eq!(
        vtable_symbols(&module.classes[class_index(1)]),
        [
            symbol_of(&module, "Base.m1"),
            symbol_of(&module, "Derived.m2"),
            symbol_of(&module, "Derived.m3")
        ]
    );
}

#[test]
fn itables_follow_the_interface_method_order() {
    let mut h = Harness::new();
    let iface = h.interface("Describable", &["a", "b"]);
    let class = h.class("C", hir::ClassModifier::Final, &[], None, &[iface]);
    let class_ty = h.class_ty(class);
    // The implementations are declared in reverse order: the
    // itable slots follow the interface's declaration order.
    let _impl_b = empty_method(&mut h, "C", "b", class_ty);
    let _impl_a = empty_method(&mut h, "C", "a", class_ty);
    // The derived class inherits `a` and overrides `b`; the
    // interface is covered without being redeclared.
    let derived = h.class(
        "D",
        hir::ClassModifier::Final,
        &[],
        Some((class, vec![])),
        &[],
    );
    let derived_ty = h.class_ty(derived);
    let _impl_b_d = empty_method(&mut h, "D", "b", derived_ty);
    let main = empty_main(&mut h);
    let module = lower(&h.finish(main));

    let class_def = &module.classes[class_index(0)];
    assert_eq!(class_def.itables.len(), 1);
    let record = &class_def.itables[0];
    assert_eq!(record.interface, la_arena::Idx::from_raw(0.into()));
    let slots: Vec<&str> = record
        .slots
        .iter()
        .map(|slot| slot_fn(&module, slot))
        .collect();
    assert_eq!(
        slots,
        [symbol_of(&module, "C.a"), symbol_of(&module, "C.b")]
    );

    let derived_def = &module.classes[class_index(1)];
    assert_eq!(derived_def.itables.len(), 1);
    let record = &derived_def.itables[0];
    let slots: Vec<&str> = record
        .slots
        .iter()
        .map(|slot| slot_fn(&module, slot))
        .collect();
    // The override dispatches to the derived implementation; the
    // inherited method keeps the base's.
    assert_eq!(
        slots,
        [symbol_of(&module, "C.a"), symbol_of(&module, "D.b")]
    );
}
