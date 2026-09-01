//! Generic nominal specialization and ordinary generic core calls.

use super::*;

#[test]
fn generic_structs_instantiate_per_argument_list() {
    let mut h = Harness::new();
    let gc = h.gc_core();
    let (string, uint) = (h.string, h.uint());
    // Two applications of one generic struct, one of them twice
    // (dedup), plus a struct whose field mentions its type
    // parameter (the general substitution path).
    let pinned_ptr_v = h.struct_app(gc.pinned_ptr, vec![uint]);
    let pinned_ptr_s = h.struct_app(gc.pinned_ptr, vec![string]);
    let t = h
        .types
        .alloc(hir::Type::Param(hir::TypeParamId::from_raw(0)));
    let box2 = h.declare_struct("Box2", vec![type_param("T")], vec![t], &[("x", t)], &[]);
    let box2_s = h.struct_app(box2, vec![string]);
    let mut locals = Arena::new();
    let a = locals.alloc(local("a", pinned_ptr_v));
    let b = locals.alloc(local("b", pinned_ptr_s));
    let c = locals.alloc(local("c", pinned_ptr_s));
    let d = locals.alloc(local("d", box2_s));
    let main = h.user_fn(
        "main",
        hir::Body {
            locals,
            statements: vec![
                val_decl(a, struct_init(&h, pinned_ptr_v, vec![int_lit(&h, 1)])),
                val_decl(b, struct_init(&h, pinned_ptr_s, vec![int_lit(&h, 2)])),
                val_decl(c, struct_init(&h, pinned_ptr_s, vec![int_lit(&h, 3)])),
                val_decl(d, struct_init(&h, box2_s, vec![str_lit(&h, "x")])),
            ],
        },
    );
    let module = lower(&h.finish(main));

    // One instance per (struct, args): `PinnedPtr$V` once,
    // `PinnedPtr$S` once despite two uses, `Box2$S` once — named
    // like the enum instances. Generic definitions themselves do
    // not survive into MIR: MIR contains no generic types.
    let defs = |name: &str| {
        module
            .structs
            .iter()
            .filter(|(_, def)| def.name == name)
            .map(|(_, def)| def)
            .collect::<Vec<_>>()
    };
    assert!(defs("PinnedPtr").is_empty());
    assert!(defs("Box2").is_empty());
    assert_eq!(defs("PinnedPtr$V").len(), 1);
    assert_eq!(
        defs("PinnedPtr$V")[0].declared_fields()[0].ty,
        mir::Type::UInt
    );
    assert!(defs("PinnedPtr$V")[0].gc_free);
    assert_eq!(defs("PinnedPtr$S").len(), 1);
    assert!(defs("PinnedPtr$S")[0].gc_free);
    assert_eq!(defs("Box2$S").len(), 1);
    // Field substitution: `Box2<String>`'s `x` is `String`.
    assert_eq!(defs("Box2$S")[0].declared_fields()[0].ty, mir::Type::String);
    assert!(!defs("Box2$S")[0].gc_free);

    // Locals and StructInits resolve to the instances.
    let body = &module.functions[module.entry].body;
    let instance_of = |local: mir::LocalId| {
        let mir::Type::Struct(id) = &body.locals[local].ty else {
            panic!("a struct local")
        };
        module.structs[*id].name.as_str()
    };
    let mir::StatementKind::ValDecl { local: la, .. } = entry_statements(body)[0].kind else {
        panic!()
    };
    let mir::StatementKind::ValDecl { local: lb, .. } = entry_statements(body)[1].kind else {
        panic!()
    };
    let mir::StatementKind::ValDecl { local: lc, .. } = entry_statements(body)[2].kind else {
        panic!()
    };
    let mir::StatementKind::ValDecl { local: ld, .. } = entry_statements(body)[3].kind else {
        panic!()
    };
    assert_eq!(instance_of(la), "PinnedPtr$V");
    assert_eq!(instance_of(lb), "PinnedPtr$S");
    assert_eq!(instance_of(lc), "PinnedPtr$S");
    assert_eq!(instance_of(ld), "Box2$S");
    let mir::StatementKind::ValDecl { init, .. } = &entry_statements(body)[0].kind else {
        panic!()
    };
    let mir::ExprKind::StructInit { struct_id, .. } = init.kind else {
        panic!("a struct construction")
    };
    assert_eq!(module.structs[struct_id].name, "PinnedPtr$V");
}

#[test]
fn generic_interface_applications_get_distinct_mir_identities() {
    let mut h = Harness::new();
    let t = h
        .types
        .alloc(hir::Type::Param(hir::TypeParamId::from_raw(0)));
    let interface = h.declare_interface(
        "Channel",
        vec![hir::TypeParamDecl {
            id: hir::TypeParamId::from_raw(0),
            name: "T".to_string(),
            variance: hir::Variance::Out,
            bounds: hir::TypeParamBounds::Unconstrained,
            span: SPAN,
        }],
        vec![t],
        Vec::new(),
    );
    let (int, string) = (h.int, h.string);
    let int_channel = h.interface_app(interface, vec![int]);
    let string_channel = h.interface_app(interface, vec![string]);
    h.declare_class(
        "Ints",
        hir::ClassModifier::Final,
        &[],
        None,
        vec![int_channel],
    );
    h.declare_class(
        "Strings",
        hir::ClassModifier::Final,
        &[],
        None,
        vec![string_channel],
    );
    let main = h.user_fn(
        "main",
        hir::Body {
            locals: Arena::new(),
            statements: Vec::new(),
        },
    );
    let module = lower(&h.finish(main));

    let names: Vec<_> = module
        .interfaces
        .iter()
        .map(|(_, interface)| interface.name.as_str())
        .collect();
    assert_eq!(names, ["Channel$I", "Channel$S"]);
    let class_interfaces: Vec<_> = module
        .classes
        .iter()
        .filter(|(_, class)| class.name == "Ints" || class.name == "Strings")
        .map(|(_, class)| class.interfaces[0])
        .collect();
    assert_ne!(class_interfaces[0], class_interfaces[1]);
}

#[test]
fn print_overloads_are_ordinary_calls() {
    // M7: calls to core's `print` / `println` overloads resolve to
    // the overload's own MIR function (`Callee::User`); only the
    // intrinsic primitives inside their bodies are runtime calls.
    let mut h = Harness::new();
    let print_string = h.print_string();
    let print_int = h.print_int();
    let print_boolean = h.print_boolean();
    let println_string = h.println_string();
    let println_int = h.println_int();
    let println_boolean = h.println_boolean();
    let main = h.user_fn(
        "main",
        hir::Body {
            locals: Arena::new(),
            statements: vec![
                expr_stmt(call(&h, print_string, vec![str_lit(&h, "s")])),
                expr_stmt(call(&h, print_int, vec![int_lit(&h, 1)])),
                expr_stmt(call(&h, print_boolean, vec![bool_lit(&h, true)])),
                expr_stmt(call(&h, println_string, vec![str_lit(&h, "t")])),
                expr_stmt(call(&h, println_int, vec![int_lit(&h, 2)])),
                expr_stmt(call(&h, println_boolean, vec![bool_lit(&h, false)])),
            ],
        },
    );
    let module = lower(&h.finish(main));

    let body = &module.functions[module.entry].body;
    let callees: Vec<mir::FunctionId> = entry_statements(body)
        .iter()
        .map(|statement| {
            let (call, _) = statement_call(statement);
            let mir::Callee::User(id) = call.target.callee else {
                panic!("print/println calls must be ordinary user calls")
            };
            id
        })
        .collect();
    // The overloads are the first six MIR functions (declaration
    // order: the three `print`s, then the three `println`s), and
    // each overload's symbol carries the parameter encoding.
    assert_eq!(callees, module.top_level[..6]);
    let symbols: Vec<&str> = callees
        .iter()
        .map(|&id| module.functions[id].symbol.as_str())
        .collect();
    assert_eq!(
        symbols,
        [
            "scoop.print.S",
            "scoop.print.I",
            "scoop.print.B",
            "scoop.println.S",
            "scoop.println.I",
            "scoop.println.B",
        ]
    );
}
