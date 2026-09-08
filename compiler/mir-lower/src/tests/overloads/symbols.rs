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
fn native_symbols_use_typed_link_stems_instead_of_display_names() {
    let mut h = Harness::new();
    let first = string_fn(&mut h, "same", &[], "a");
    let second = string_fn(&mut h, "same", &[], "b");
    h.functions[first].link_stem = callable_link_stem("pkg.a.same");
    h.functions[second].link_stem = callable_link_stem("pkg.b.same");
    let main = empty_main(&mut h);
    let module = lower(&h.finish(main));

    assert_eq!(
        top_level_symbols(&module),
        ["scoop.pkg.a.same", "scoop.pkg.b.same", "scoop_main"]
    );
    assert_eq!(
        module
            .top_level
            .iter()
            .filter(|&&id| module.functions[id].name == "same")
            .count(),
        2,
        "the source-facing display name remains unchanged"
    );
}

#[test]
fn generic_instance_mangling_uses_the_propagated_link_stem() {
    let mut h = Harness::new();
    let int = h.int;
    let generic = identity_fn(&mut h, "identity");
    h.functions[generic].link_stem = callable_link_stem("pkg.a.identity");
    let instance = h.instantiate(generic, vec![int]);
    let main = h.user_fn(
        "main",
        hir::Body {
            locals: Arena::new(),
            statements: vec![expr_stmt(generic_call(instance, vec![int_lit(&h, 1)], int))],
        },
    );
    let module = lower(&h.finish(main));

    assert!(
        top_level_symbols(&module).contains(&"scoop.pkg.a.identity$I32"),
        "the generic instance symbol must start from the typed link stem"
    );
}

#[test]
fn extension_receiver_parameters_disambiguate_a_shared_extension_stem() {
    let mut h = Harness::new();
    let (int, string) = (h.int, h.string);
    let int_extension = string_fn(&mut h, "same", &[("this", int)], "int");
    let string_extension = string_fn(&mut h, "same", &[("this", string)], "string");
    let shared = callable_link_stem("pkg.extension.same");
    h.functions[int_extension].link_stem = shared.clone();
    h.functions[string_extension].link_stem = shared;
    let main = empty_main(&mut h);
    let module = lower(&h.finish(main));

    assert_eq!(
        top_level_symbols(&module),
        [
            "scoop.pkg.extension.same.I32",
            "scoop.pkg.extension.same.S",
            "scoop_main"
        ]
    );
}

#[test]
fn constructor_symbols_use_nominal_stems_instead_of_display_names() {
    let mut h = Harness::new();
    let class_a = h.class("Same", hir::ClassModifier::Final, &[], None, &[]);
    let class_b = h.class("Same", hir::ClassModifier::Final, &[], None, &[]);
    h.classes[class_a].link_stem = nominal_link_stem("$pkg$a$class$Same");
    h.classes[class_b].link_stem = nominal_link_stem("$pkg$b$class$Same");
    let struct_a = h.strukt("Value", &[]);
    let struct_b = h.strukt("Value", &[]);
    h.structs[struct_a].link_stem = nominal_link_stem("$pkg$a$struct$Value");
    h.structs[struct_b].link_stem = nominal_link_stem("$pkg$b$struct$Value");
    let main = empty_main(&mut h);
    let module = lower(&h.finish(main));

    let class_symbols = module
        .functions
        .iter()
        .filter(|(_, function)| function.name.starts_with("init.Same.$c"))
        .map(|(_, function)| function.symbol.as_str())
        .collect::<Vec<_>>();
    assert_eq!(class_symbols.len(), 2);
    for (id, definition) in module.classes.iter().take(2) {
        let owner = mir::encode_type(&module, &mir::Type::Class(id)).unwrap();
        assert!(
            class_symbols
                .iter()
                .any(|symbol| symbol.starts_with(&format!("scoop.init.{owner}.$c"))),
            "missing constructor symbol for {}",
            definition.link_stem.as_str()
        );
    }

    let struct_symbols = module
        .functions
        .iter()
        .filter(|(_, function)| function.name.starts_with("ctor.Value.$c"))
        .map(|(_, function)| function.symbol.as_str())
        .collect::<Vec<_>>();
    assert_eq!(struct_symbols.len(), 2);
    for (id, definition) in module.structs.iter().take(2) {
        let owner = mir::encode_type(&module, &mir::Type::Struct(id)).unwrap();
        assert!(
            struct_symbols
                .iter()
                .any(|symbol| symbol.starts_with(&format!("scoop.ctor.{owner}.$c"))),
            "missing constructor symbol for {}",
            definition.link_stem.as_str()
        );
    }
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
