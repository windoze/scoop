//! Class, interface, boxing and reference-operation lowering.

use super::*;

/// A `this`-taking method with an empty body, as hir-lower
/// produces it for `fun m() {}`-style declarations; the name is
/// qualified `Owner.method` like hir-lower qualifies members.
fn empty_method(
    h: &mut Harness,
    owner: &str,
    name: &str,
    receiver: hir::TypeId,
) -> hir::FunctionId {
    let mut locals = Arena::new();
    let this = locals.alloc(local("this", receiver));
    let unit = h.unit;
    h.method_fn(
        &format!("{owner}.{name}"),
        receiver,
        vec![param("this", receiver, this)],
        unit,
        hir::Body {
            locals,
            statements: Vec::new(),
        },
    )
}

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
                            index: 1,
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
        ["scoop.Base.m1", "scoop.Base.m2"]
    );
    // The base prefix is preserved; the override replaces slot 1
    // in place; the new method appends at slot 2.
    assert_eq!(
        vtable_symbols(&module.classes[class_index(1)]),
        ["scoop.Base.m1", "scoop.Derived.m2", "scoop.Derived.m3"]
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
    assert_eq!(slots, ["scoop.C.a", "scoop.C.b"]);

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
    assert_eq!(slots, ["scoop.C.a", "scoop.D.b"]);
}

#[test]
fn method_calls_are_annotated_by_the_receiver_static_type() {
    let mut h = Harness::new();
    let iface = h.interface("Describable", &["describe", "label"]);
    let iface_ty = h.interface_ty(iface);
    let class = h.class("C", hir::ClassModifier::Open, &[], None, &[iface]);
    let class_ty = h.class_ty(class);
    let class_describe = empty_method(&mut h, "C", "describe", class_ty);
    let _class_label = empty_method(&mut h, "C", "label", class_ty);
    // Interface method shells, as hir-lower materializes them.
    let _iface_describe = empty_method(&mut h, "Describable", "describe", iface_ty);
    let iface_label = empty_method(&mut h, "Describable", "label", iface_ty);
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
                callee: hir::MethodCallee::Callable(hir::Callable::Method(application)),
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

    let body = &module.functions[module.entry].body;
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
                callee: hir::MethodCallee::Callable(hir::Callable::Method(application)),
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
    assert_eq!(slot_fn(&module, &base_vtable[0]), "scoop.Base.openMethod");
    let derived_vtable = &module.classes[class_index(1)].vtable;
    assert_eq!(derived_vtable.len(), 1);
    assert_eq!(
        slot_fn(&module, &derived_vtable[0]),
        "scoop.Derived.openMethod"
    );

    let body = &module.functions[module.entry].body;
    let kind = |index: usize| {
        let (call, _) = statement_call(&entry_statements(body)[index]);
        &call.target.kind
    };
    assert!(matches!(kind(0), mir::CallKind::Virtual { slot: 0 }));
    assert!(matches!(kind(1), mir::CallKind::Direct));
    assert!(matches!(kind(2), mir::CallKind::Direct));
}

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
    assert_eq!(thunk_symbol, "scoop.thunk.D1_SX.Describable.describe");

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
    assert_eq!(module.functions[impl_id].symbol, "scoop.S.describe");
    assert_eq!(call.args.len(), 1);
    assert!(matches!(&call.args[0].kind, mir::ExprKind::Unbox(operand)
            if matches!(operand.kind, mir::ExprKind::Local(local) if local == thunk.params[0].local)));
}

#[test]
fn is_instance_and_casts_lower_to_runtime_checks() {
    let mut h = Harness::new();
    h.exception("ClassCastException");
    let (int, boolean) = (h.int, h.boolean);
    let s = h.strukt("S", &[("x", int)]);
    let s_ty = h.struct_ty(s);
    let any = h.any();
    let option_s = h.option(s_ty);
    let mut locals = Arena::new();
    let a = locals.alloc(local("a", any));
    let is_s = locals.alloc(local("is_s", boolean));
    let s2 = locals.alloc(local("s2", s_ty));
    let maybe = locals.alloc(local("maybe", option_s));
    let main = h.user_fn(
        "main",
        hir::Body {
            locals,
            statements: vec![
                val_decl(
                    is_s,
                    expr(
                        hir::ExprKind::IsInstance {
                            operand: Box::new(local_ref(a, any)),
                            check_ty: s_ty,
                        },
                        boolean,
                    ),
                ),
                val_decl(
                    s2,
                    // Mirror hir-lower's real shape: a value-typed
                    // `as` arrives as `Unbox(Cast)`; mir-lower's
                    // cast expansion only performs the check.
                    expr(
                        hir::ExprKind::Unbox(Box::new(expr(
                            hir::ExprKind::Cast {
                                operand: Box::new(local_ref(a, any)),
                                optional: false,
                            },
                            s_ty,
                        ))),
                        s_ty,
                    ),
                ),
                val_decl(
                    maybe,
                    expr(
                        hir::ExprKind::Cast {
                            operand: Box::new(local_ref(a, any)),
                            optional: true,
                        },
                        option_s,
                    ),
                ),
            ],
        },
    );
    let module = lower(&h.finish(main));

    // `is` stays a dedicated node; `as` throws
    // `ClassCastException` on failure (M8); `as?` wraps in
    // Some / None. The value-type checks registered the boxed
    // payload class. Capabilities are not synthesized from boxing.
    let expected = "\
Module
  struct S (x: Int)
  enum Option$D1_SX
    Some(_1: S)
    None()
  class ClassCastException vtable=0 itables=0
  class box$D1_SX vtable=0 itables=0
  fun main @scoop_main() -> Unit
    bb0 entry
      val is_s: Boolean
        Type Boolean
        IsInstance S
          Type Any
          Local a
      val $cast.1: Any
        Type Any
        Local a
      branch bb1 bb2
        Type Boolean
        Unary BoolNot
          Type Boolean
          IsInstance S
            Type Any
            Local $cast.1
    bb1 if.then.1
      call $call.1: ClassCastException = @scoop.ctor.ClassCastException direct
      throw
        Type ClassCastException
        Local $call.1
    bb2 if.merge.2
      val $ub.2: S
        Type S
        Unbox
          Type Any
          Local $cast.1
      val s2: S
        Type S
        Local $ub.2
      val $cast.3: Any
        Type Any
        Local a
      branch bb3 bb4
        Type Boolean
        IsInstance S
          Type Any
          Local $cast.3
    bb3 if.then.3
      assign $cast.4
        Type Option$D1_SX<S>
        VariantConstruct Option$D1_SX<S> v0
          Type S
          Unbox
            Type Any
            Local $cast.3
      goto bb5
    bb4 if.else.4
      assign $cast.4
        Type Option$D1_SX<S>
        VariantConstruct Option$D1_SX<S> v1
      goto bb5
    bb5 if.merge.5
      val maybe: Option$D1_SX<S>
        Type Option$D1_SX<S>
        Local $cast.4
      return
  fun ctor.ClassCastException @scoop.ctor.ClassCastException() -> ClassCastException
    bb0 entry
      return
        Type ClassCastException
        ClassInit ClassCastException
  entry @scoop_main
";
    assert_eq!(dump(&module), expected);
}

#[test]
fn constructor_functions_initialize_the_flattened_fields() {
    // open class Root(val label: String)
    // open class Base(val name: String) : Root("root")
    // class Point(val x: Int) : Base("point")
    let mut h = Harness::new();
    let (int, string) = (h.int, h.string);
    let root = h.class(
        "Root",
        hir::ClassModifier::Open,
        &[("label", string)],
        None,
        &[],
    );
    let base = h.class(
        "Base",
        hir::ClassModifier::Open,
        &[("name", string)],
        Some((root, vec![str_lit(&h, "root")])),
        &[],
    );
    let point = h.class(
        "Point",
        hir::ClassModifier::Final,
        &[("x", int)],
        Some((base, vec![str_lit(&h, "point")])),
        &[],
    );
    let point_ty = h.class_ty(point);
    let point_application = h.class_application_of(point_ty);
    let mut locals = Arena::new();
    let p = locals.alloc(local("p", point_ty));
    let main = h.user_fn(
        "main",
        hir::Body {
            locals,
            statements: vec![val_decl(
                p,
                expr(
                    hir::ExprKind::ClassInit {
                        application: point_application,
                        args: vec![int_lit(&h, 1)],
                    },
                    point_ty,
                ),
            )],
        },
    );
    let module = lower(&h.finish(main));

    // One ctor per class; the use site is a plain direct call.
    // Each ctor returns a raw ClassInit over the flattened field
    // values: the base delegation arguments (re-evaluated in each
    // derived ctor — hence the repeated "root" constant), then the
    // own properties. No base ctor is called.
    let expected = "\
Module
  class Root vtable=0 itables=0
  class Base vtable=0 itables=0
  class Point vtable=0 itables=0
  fun main @scoop_main() -> Unit
    bb0 entry
      call p: Point = @scoop.ctor.Point direct
        Type Int
        IntLiteral 1
      return
  fun ctor.Root @scoop.ctor.Root(label: String) -> Root
    bb0 entry
      return
        Type Root
        ClassInit Root
          Type String
          Local label
  fun ctor.Base @scoop.ctor.Base(name: String) -> Base
    bb0 entry
      return
        Type Base
        ClassInit Base
          Type String
          StringConst @scoop.str.0
          Type String
          Local name
  fun ctor.Point @scoop.ctor.Point(x: Int) -> Point
    bb0 entry
      return
        Type Point
        ClassInit Point
          Type String
          StringConst @scoop.str.2
          Type String
          StringConst @scoop.str.1
          Type Int
          Local x
  str @scoop.str.0 \"root\"
  str @scoop.str.1 \"point\"
  str @scoop.str.2 \"root\"
  entry @scoop_main
";
    assert_eq!(dump(&module), expected);
}

#[test]
fn abstract_classes_get_no_constructor() {
    let mut h = Harness::new();
    let _base = h.class("Base", hir::ClassModifier::Abstract, &[], None, &[]);
    let main = empty_main(&mut h);
    let module = lower(&h.finish(main));

    assert!(
        !module
            .functions
            .iter()
            .any(|(_, f)| f.symbol == "scoop.ctor.Base")
    );
}

#[test]
fn field_assignment_lowers_to_field_set() {
    // `p.y = 3` on a class with two properties (index 1 in the
    // flattened layout).
    let mut h = Harness::new();
    let int = h.int;
    let c = h.class(
        "C",
        hir::ClassModifier::Final,
        &[("x", int), ("y", int)],
        None,
        &[],
    );
    let c_ty = h.class_ty(c);
    let c_application = h.class_application_of(c_ty);
    let mut locals = Arena::new();
    let p = locals.alloc(local("p", c_ty));
    let main = h.user_fn(
        "main",
        hir::Body {
            locals,
            statements: vec![stmt(hir::StatementKind::Assign {
                target: hir::AssignTarget::Field {
                    receiver: Box::new(local_ref(p, c_ty)),
                    field: hir::FieldRef::ClassField {
                        application: c_application,
                        index: 1,
                    },
                },
                value: int_lit(&h, 3),
            })],
        },
    );
    let module = lower(&h.finish(main));

    let body = &module.functions[module.entry].body;
    let mir::StatementKind::FieldSet {
        object,
        index: 1,
        value,
    } = &entry_statements(body)[0].kind
    else {
        panic!("a class property assignment must lower to FieldSet")
    };
    assert!(matches!(object.kind, mir::ExprKind::Local(_)));
    assert!(matches!(value.kind, mir::ExprKind::IntLiteral(3)));
}

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
    assert_eq!(op_of(0), mir::BinOp::IntEq);
    assert_eq!(op_of(1), mir::BinOp::IntNe);
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
    assert_eq!(slot_fn(&module, &base_def.vtable[0]), "scoop.Base.id");
    let stub = module
        .functions
        .iter()
        .map(|(_, f)| f)
        .find(|f| f.symbol == "scoop.Base.id")
        .expect("the abstract method is emitted");
    assert!(
        module
            .top_level
            .iter()
            .any(|&id| module.functions[id].symbol == "scoop.Base.id")
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
        "scoop.Doc.describe"
    );
    // The implementing method is a vtable method too.
    assert_eq!(slot_fn(&module, &doc_def.vtable[0]), "scoop.Doc.describe");
}

#[test]
fn smart_cast_unboxes_bind_typed_hidden_locals() {
    // `if (a is S) { println(a.v) }` — the narrowed read arrives as
    // `FieldAccess { receiver: Unbox(Local a) }` (hir-lower's smart
    // cast). The unbox must be bound to a typed hidden local so LIR
    // never has to reconstruct its type from the `Any` operand.
    let mut h = Harness::new();
    let println_int = h.println_int();
    let (int, boolean) = (h.int, h.boolean);
    let s = h.strukt("S", &[("v", int)]);
    let s_ty = h.struct_ty(s);
    let s_application = h.struct_application_of(s_ty);
    let any = h.any();
    let mut locals = Arena::new();
    let a = locals.alloc(local("a", any));
    let print_call = expr(
        hir::ExprKind::Call {
            callee: hir::Callable::Function(println_int),
            args: vec![expr(
                hir::ExprKind::FieldAccess {
                    receiver: Box::new(expr(
                        hir::ExprKind::Unbox(Box::new(local_ref(a, any))),
                        s_ty,
                    )),
                    field: hir::FieldRef::StructField {
                        application: s_application,
                        index: 0,
                    },
                },
                int,
            )],
        },
        h.unit,
    );
    let main = h.user_fn(
        "main",
        hir::Body {
            locals,
            statements: vec![stmt(hir::StatementKind::If {
                cond: expr(
                    hir::ExprKind::IsInstance {
                        operand: Box::new(local_ref(a, any)),
                        check_ty: s_ty,
                    },
                    boolean,
                ),
                then_body: vec![expr_stmt(print_call)],
                else_body: None,
            })],
        },
    );
    let module = lower(&h.finish(main));

    let body = &module.functions[module.entry].body;
    let mir::Terminator::Branch { then_block, .. } = body.blocks[body.entry].terminator else {
        panic!("expected a conditional branch")
    };
    let then_body = &body.blocks[then_block].statements;
    let mir::StatementKind::ValDecl { local: ub, init } = &then_body[0].kind else {
        panic!("the unbox must be a val declaration")
    };
    let mir::ExprKind::Unbox(_) = init.kind else {
        panic!("the unbox must be bound to a typed hidden local")
    };
    assert_eq!(
        body.locals[*ub].ty,
        mir::Type::Struct(la_arena::Idx::from_raw(0.into()))
    );
    let (call, _) = statement_call(&then_body[1]);
    assert!(
        matches!(&call.args[0].kind, mir::ExprKind::FieldAccess { receiver, .. }
            if matches!(receiver.kind, mir::ExprKind::Local(local) if local == *ub))
    );
}
