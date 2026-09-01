//! Overload mangling and typed overload dispatch.

use super::*;

/// `fun <name>(<params>): String = <text>` — one overload each.
fn string_fn(
    h: &mut Harness,
    name: &str,
    params: &[(&str, hir::TypeId)],
    text: &str,
) -> hir::FunctionId {
    let string = h.string;
    let mut locals = Arena::new();
    let params: Vec<hir::Param> = params
        .iter()
        .map(|(name, ty)| {
            let local_id = locals.alloc(local(name, *ty));
            param(name, *ty, local_id)
        })
        .collect();
    let init = str_lit(h, text);
    h.user_fn_full(
        name,
        Vec::new(),
        params,
        string,
        hir::Body {
            locals,
            statements: vec![stmt(hir::StatementKind::Return { value: Some(init) })],
        },
    )
}

fn top_level_symbols(module: &mir::Module) -> Vec<&str> {
    module
        .top_level
        .iter()
        .map(|&id| module.functions[id].symbol.as_str())
        .collect()
}

#[test]
fn overloads_mangle_with_param_encoding() {
    let mut h = Harness::new();
    let (int, string) = (h.int, h.string);
    string_fn(&mut h, "show", &[("v", int)], "int");
    string_fn(&mut h, "show", &[("v", string)], "string");
    string_fn(&mut h, "show", &[("v", int), ("extra", int)], "two");
    // A unique name keeps the plain `scoop.<name>` symbol.
    string_fn(&mut h, "helper", &[], "h");
    let main = empty_main(&mut h);
    let module = lower(&h.finish(main));

    assert_eq!(
        top_level_symbols(&module),
        [
            "scoop.show.I",
            "scoop.show.S",
            "scoop.show.I_I",
            "scoop.helper",
            "scoop_main"
        ]
    );
}

#[test]
fn zero_parameter_overload_mangles_with_an_empty_encoding() {
    let mut h = Harness::new();
    let int = h.int;
    string_fn(&mut h, "f", &[], "none");
    string_fn(&mut h, "f", &[("v", int)], "one");
    let main = empty_main(&mut h);
    let module = lower(&h.finish(main));

    assert_eq!(
        top_level_symbols(&module),
        ["scoop.f.", "scoop.f.I", "scoop_main"]
    );
}

#[test]
fn overload_symbols_do_not_collide_with_instance_symbols() {
    // `show(Int)` / `show(String)` overloads plus a generic
    // `show<T>` instantiated with `Int`: `.` vs `$` keep the
    // symbols distinct.
    let mut h = Harness::new();
    let (int, string) = (h.int, h.string);
    string_fn(&mut h, "show", &[("v", int)], "int");
    string_fn(&mut h, "show", &[("v", string)], "string");
    let generic = identity_fn(&mut h, "show");
    let generic_int = h.instantiate(generic, vec![int]);
    let main = h.user_fn(
        "main",
        hir::Body {
            locals: Arena::new(),
            statements: vec![expr_stmt(generic_call(
                generic_int,
                vec![int_lit(&h, 1)],
                int,
            ))],
        },
    );
    let module = lower(&h.finish(main));

    let symbols = top_level_symbols(&module);
    for expected in ["scoop.show.I", "scoop.show.S", "scoop.show$I"] {
        assert!(
            symbols.contains(&expected),
            "missing {expected} in {symbols:?}"
        );
    }
}

#[test]
fn method_overloads_mangle_with_param_encoding() {
    // `class Doc { fun describe(v: Int); fun describe(v: String) }`:
    // the receiver is not part of the overload encoding.
    let mut h = Harness::new();
    let (int, string, unit) = (h.int, h.string, h.unit);
    let doc = h.class("Doc", hir::ClassModifier::Final, &[], None, &[]);
    let doc_ty = h.class_ty(doc);
    for ty in [int, string] {
        let mut locals = Arena::new();
        let this = locals.alloc(local("this", doc_ty));
        let v = locals.alloc(local("v", ty));
        h.method_fn(
            "Doc.describe",
            doc_ty,
            vec![param("this", doc_ty, this), param("v", ty, v)],
            unit,
            hir::Body {
                locals,
                statements: Vec::new(),
            },
        );
    }
    let main = empty_main(&mut h);
    let module = lower(&h.finish(main));

    let symbols: std::collections::HashSet<&str> = module
        .functions
        .iter()
        .map(|(_, f)| f.symbol.as_str())
        .collect();
    assert!(symbols.contains("scoop.Doc.describe.I"));
    assert!(symbols.contains("scoop.Doc.describe.S"));
    // Each overload gets its own vtable slot (keyed by signature),
    // referencing the final (overload-encoded) symbol by id.
    let doc_def = &module.classes[class_index(0)];
    assert_eq!(doc_def.vtable.len(), 2);
    assert_eq!(slot_fn(&module, &doc_def.vtable[0]), "scoop.Doc.describe.I");
    assert_eq!(slot_fn(&module, &doc_def.vtable[1]), "scoop.Doc.describe.S");
}

/// A class method returning an Int constant:
/// `fun <owner>.<name>(v: <param_ty>): Int = <value>` (param
/// optional). Returns the HIR function id.
fn int_method(
    h: &mut Harness,
    qualified: &str,
    receiver: hir::TypeId,
    param_ty: Option<hir::TypeId>,
    value: i64,
) -> hir::FunctionId {
    let int = h.int;
    let mut locals = Arena::new();
    let this = locals.alloc(local("this", receiver));
    let mut params = vec![param("this", receiver, this)];
    if let Some(ty) = param_ty {
        let v = locals.alloc(local("v", ty));
        params.push(param("v", ty, v));
    }
    h.method_fn(
        qualified,
        receiver,
        params,
        int,
        hir::Body {
            locals,
            statements: vec![stmt(hir::StatementKind::Return {
                value: Some(int_lit(h, value)),
            })],
        },
    )
}

#[test]
fn overridden_overload_replaces_the_base_slot_in_place() {
    // open class A { fun s(v: Int): Int = 1; fun s(v: String): Int = 2 }
    // class B : A() { override fun s(v: Int): Int = 3 }
    let mut h = Harness::new();
    let (int, string) = (h.int, h.string);
    let a = h.class("A", hir::ClassModifier::Open, &[], None, &[]);
    let a_ty = h.class_ty(a);
    int_method(&mut h, "A.s", a_ty, Some(int), 1);
    int_method(&mut h, "A.s", a_ty, Some(string), 2);
    let b = h.class(
        "B",
        hir::ClassModifier::Final,
        &[],
        Some((a, Vec::new())),
        &[],
    );
    let b_ty = h.class_ty(b);
    int_method(&mut h, "B.s", b_ty, Some(int), 3);
    let main = empty_main(&mut h);
    let module = lower(&h.finish(main));

    // A: one slot per overload.
    let a_def = &module.classes[class_index(0)];
    assert_eq!(a_def.vtable.len(), 2);
    assert_eq!(slot_fn(&module, &a_def.vtable[0]), "scoop.A.s.I");
    assert_eq!(slot_fn(&module, &a_def.vtable[1]), "scoop.A.s.S");
    // B: the `s(Int)` override replaces slot 0 in place; the
    // inherited `s(String)` keeps slot 1. (`B.s` is a unique name
    // in the module, so it keeps the plain symbol.)
    let b_def = &module.classes[class_index(1)];
    assert_eq!(b_def.vtable.len(), 2);
    assert_eq!(slot_fn(&module, &b_def.vtable[0]), "scoop.B.s");
    assert_eq!(slot_fn(&module, &b_def.vtable[1]), "scoop.A.s.S");
}

#[test]
fn virtual_calls_annotate_the_overloads_own_slot() {
    // `val a: A = ...; a.s(1); a.s("x")` — the callee is the
    // signature resolved on the static type; each call annotates
    // its own overload's slot.
    let mut h = Harness::new();
    let (int, string, unit) = (h.int, h.string, h.unit);
    let a = h.class("A", hir::ClassModifier::Open, &[], None, &[]);
    let a_ty = h.class_ty(a);
    let s_int = int_method(&mut h, "A.s", a_ty, Some(int), 1);
    let s_string = int_method(&mut h, "A.s", a_ty, Some(string), 2);
    let s_int = h.method_application(s_int);
    let s_string = h.method_application(s_string);
    let method_call =
        |receiver: hir::Expr, application: hir::MethodApplicationId, arg: hir::Expr| {
            expr(
                hir::ExprKind::MethodCall {
                    receiver: Box::new(receiver),
                    callee: hir::MethodCallee::Callable(hir::Callable::Method(application)),
                    args: vec![arg],
                },
                unit,
            )
        };
    let mut locals = Arena::new();
    let av = locals.alloc(local("a", a_ty));
    let main = h.user_fn(
        "main",
        hir::Body {
            locals,
            statements: vec![
                expr_stmt(method_call(local_ref(av, a_ty), s_int, int_lit(&h, 1))),
                expr_stmt(method_call(local_ref(av, a_ty), s_string, str_lit(&h, "x"))),
            ],
        },
    );
    let module = lower(&h.finish(main));

    let body = &module.functions[module.entry].body;
    let call_kind = |index: usize| {
        let (call, _) = statement_call(&entry_statements(body)[index]);
        &call.target.kind
    };
    assert!(matches!(call_kind(0), mir::CallKind::Virtual { slot: 0 }));
    assert!(matches!(call_kind(1), mir::CallKind::Virtual { slot: 1 }));
}

/// `interface <name> { fun m(v: T)... }` — one `MethodSig` per
/// `(name, param type)` entry, as hir-lower materializes them
/// (interface methods carry no `this` in the signature).
fn overloaded_interface(
    h: &mut Harness,
    name: &str,
    methods: &[(&str, hir::TypeId)],
) -> hir::InterfaceId {
    let return_ty = h.int;
    let mut locals = Arena::new();
    let methods = methods
        .iter()
        .map(|(name, ty)| {
            let v = locals.alloc(local("v", *ty));
            hir::MethodSig {
                name: name.to_string(),
                is_suspend: false,
                attributes: hir::FunctionAttributes::default(),
                type_params: Vec::new(),
                params: vec![param("v", *ty, v)],
                return_ty,
                span: SPAN,
            }
        })
        .collect();
    h.declare_interface(name, Vec::new(), Vec::new(), methods)
}

#[test]
fn overloaded_interface_methods_get_one_itable_slot_each() {
    // interface Multi { fun m(v: Int); fun m(v: String) }
    // class C : Multi implements both overloads.
    let mut h = Harness::new();
    let (int, string) = (h.int, h.string);
    let multi = overloaded_interface(&mut h, "Multi", &[("m", int), ("m", string)]);
    let c = h.class("C", hir::ClassModifier::Final, &[], None, &[multi]);
    let c_ty = h.class_ty(c);
    int_method(&mut h, "C.m", c_ty, Some(int), 1);
    int_method(&mut h, "C.m", c_ty, Some(string), 2);
    let main = empty_main(&mut h);
    let module = lower(&h.finish(main));

    let c_def = &module.classes[class_index(0)];
    assert_eq!(c_def.itables.len(), 1);
    let record = &c_def.itables[0];
    assert_eq!(record.slots.len(), 2);
    assert_eq!(slot_fn(&module, &record.slots[0]), "scoop.C.m.I");
    assert_eq!(slot_fn(&module, &record.slots[1]), "scoop.C.m.S");
}

#[test]
fn interface_calls_annotate_the_overloads_own_slot() {
    // `val i: Multi = ...; i.m(1); i.m("x")` — interface dispatch
    // locates the slot by the callee's signature.
    let mut h = Harness::new();
    let (int, string) = (h.int, h.string);
    let multi = overloaded_interface(&mut h, "Multi", &[("m", int), ("m", string)]);
    let multi_ty = h.interface_ty(multi);
    // Interface method shells, as hir-lower materializes them
    // (params include `this`).
    let shell = |h: &mut Harness, ty: hir::TypeId| {
        let mut locals = Arena::new();
        let this = locals.alloc(local("this", multi_ty));
        let v = locals.alloc(local("v", ty));
        h.method_fn(
            "Multi.m",
            multi_ty,
            vec![param("this", multi_ty, this), param("v", ty, v)],
            int,
            hir::Body {
                locals,
                statements: Vec::new(),
            },
        )
    };
    let m_int = shell(&mut h, int);
    let m_string = shell(&mut h, string);
    let m_int = h.method_application(m_int);
    let m_string = h.method_application(m_string);
    let method_call =
        |receiver: hir::Expr, application: hir::MethodApplicationId, arg: hir::Expr| {
            expr(
                hir::ExprKind::MethodCall {
                    receiver: Box::new(receiver),
                    callee: hir::MethodCallee::Callable(hir::Callable::Method(application)),
                    args: vec![arg],
                },
                int,
            )
        };
    let mut locals = Arena::new();
    let i = locals.alloc(local("i", multi_ty));
    let main = h.user_fn(
        "main",
        hir::Body {
            locals,
            statements: vec![
                expr_stmt(method_call(local_ref(i, multi_ty), m_int, int_lit(&h, 1))),
                expr_stmt(method_call(
                    local_ref(i, multi_ty),
                    m_string,
                    str_lit(&h, "x"),
                )),
            ],
        },
    );
    let module = lower(&h.finish(main));

    let body = &module.functions[module.entry].body;
    let call_kind = |index: usize| {
        let (call, _) = statement_call(&entry_statements(body)[index]);
        &call.target.kind
    };
    let is_iface_slot = |kind: &mir::CallKind, slot: u32| matches!(kind, mir::CallKind::Interface { slot: s, .. } if *s == slot);
    assert!(is_iface_slot(call_kind(0), 0));
    assert!(is_iface_slot(call_kind(1), 1));
}

#[test]
fn boxed_thunks_of_overloaded_interface_methods_are_disambiguated() {
    // struct S : Multi implements both `m` overloads; boxing to
    // `Multi` generates one thunk per signature.
    let mut h = Harness::new();
    let (int, string) = (h.int, h.string);
    let multi = overloaded_interface(&mut h, "Multi", &[("m", int), ("m", string)]);
    let multi_ty = h.interface_ty(multi);
    let s = h.strukt_with("S", &[("x", int)], &[multi]);
    let s_ty = h.struct_ty(s);
    int_method(&mut h, "S.m", s_ty, Some(int), 1);
    int_method(&mut h, "S.m", s_ty, Some(string), 2);
    let mut locals = Arena::new();
    let d = locals.alloc(local("d", multi_ty));
    let main = h.user_fn(
        "main",
        hir::Body {
            locals,
            statements: vec![val_decl(
                d,
                expr(
                    hir::ExprKind::Box(Box::new(struct_init(&h, s_ty, vec![int_lit(&h, 1)]))),
                    multi_ty,
                ),
            )],
        },
    );
    let module = lower(&h.finish(main));

    let boxed = boxed_class(&module, "box$D1_SX");
    assert_eq!(boxed.itables.len(), 1);
    let record = &boxed.itables[0];
    assert_eq!(record.slots.len(), 2);
    assert_eq!(
        slot_fn(&module, &record.slots[0]),
        "scoop.thunk.D1_SX.Multi.m.I"
    );
    assert_eq!(
        slot_fn(&module, &record.slots[1]),
        "scoop.thunk.D1_SX.Multi.m.S"
    );
    // Each thunk tail-calls its own overload.
    let thunk_target = |slot: &mir::TableSlot| {
        let symbol = slot_fn(&module, slot);
        let thunk = module
            .functions
            .iter()
            .map(|(_, f)| f)
            .find(|f| f.symbol == symbol)
            .expect("the thunk is a MIR function");
        let (call, _) = statement_call(&entry_statements(&thunk.body)[0]);
        let mir::Callee::User(target) = call.target.callee else {
            panic!("the thunk calls a user function")
        };
        module.functions[target].symbol.clone()
    };
    assert_eq!(thunk_target(&record.slots[0]), "scoop.S.m.I");
    assert_eq!(thunk_target(&record.slots[1]), "scoop.S.m.S");
}
