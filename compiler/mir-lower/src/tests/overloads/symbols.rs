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
            "scoop.show.I32",
            "scoop.show.S",
            "scoop.show.I32_I32",
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
        ["scoop.f.", "scoop.f.I32", "scoop_main"]
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
    for expected in ["scoop.show.I32", "scoop.show.S", "scoop.show$I32"] {
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
    assert!(symbols.contains("scoop.Doc.describe.I32"));
    assert!(symbols.contains("scoop.Doc.describe.S"));
    // Each overload gets its own vtable slot (keyed by signature),
    // referencing the final (overload-encoded) symbol by id.
    let doc_def = &module.classes[class_index(0)];
    assert_eq!(doc_def.vtable.len(), 2);
    assert_eq!(
        slot_fn(&module, &doc_def.vtable[0]),
        "scoop.Doc.describe.I32"
    );
    assert_eq!(slot_fn(&module, &doc_def.vtable[1]), "scoop.Doc.describe.S");
}
