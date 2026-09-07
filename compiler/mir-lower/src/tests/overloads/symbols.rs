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
fn overloads_get_distinct_persistent_symbols() {
    let mut h = Harness::new();
    let (int, string) = (h.int, h.string);
    string_fn(&mut h, "show", &[("v", int)], "int");
    string_fn(&mut h, "show", &[("v", string)], "string");
    string_fn(&mut h, "show", &[("v", int), ("extra", int)], "two");
    string_fn(&mut h, "helper", &[], "h");
    let main = empty_main(&mut h);
    let module = lower(&h.finish(main));

    // Every top-level symbol is persistent-mangled; the three `show`
    // overloads differ by signature key, so all symbols are distinct.
    let show_symbols: std::collections::HashSet<&str> = module
        .functions
        .iter()
        .filter(|(_, f)| f.name == "show")
        .map(|(_, f)| f.symbol.as_str())
        .collect();
    assert_eq!(show_symbols.len(), 3);
    let mut expected: std::collections::HashSet<&str> = show_symbols.clone();
    expected.insert(symbol_of(&module, "helper"));
    expected.insert("scoop_main");
    let actual: std::collections::HashSet<&str> = top_level_symbols(&module).into_iter().collect();
    assert_eq!(actual, expected);
    assert!(show_symbols.iter().all(|s| s.starts_with("scoop$1$fn$")));
}

#[test]
fn zero_parameter_overload_gets_its_own_symbol() {
    let mut h = Harness::new();
    let int = h.int;
    string_fn(&mut h, "f", &[], "none");
    string_fn(&mut h, "f", &[("v", int)], "one");
    let main = empty_main(&mut h);
    let module = lower(&h.finish(main));

    assert_eq!(
        top_level_symbols(&module),
        [
            symbol_of_arity(&module, "f", 0),
            symbol_of_arity(&module, "f", 1),
            "scoop_main"
        ]
    );
    assert_ne!(
        symbol_of_arity(&module, "f", 0),
        symbol_of_arity(&module, "f", 1)
    );
}

#[test]
fn overload_symbols_do_not_collide_with_instance_symbols() {
    // `show(Int)` / `show(String)` overloads plus a generic
    // `show<T>` instantiated with `Int`: signature keys and ODR
    // specialization keys keep the symbols distinct.
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
    // The two overloads and the generic instance are three distinct
    // persistent symbols all named `show`.
    let show_symbols: Vec<&str> = symbols
        .iter()
        .copied()
        .filter(|symbol| {
            module
                .functions
                .iter()
                .any(|(_, f)| f.symbol == *symbol && f.name == "show")
        })
        .collect();
    assert_eq!(show_symbols.len(), 3, "in {symbols:?}");
    let unique: std::collections::HashSet<&str> = show_symbols.into_iter().collect();
    assert_eq!(unique.len(), 3);
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

    let describe_symbols: std::collections::HashSet<&str> = module
        .functions
        .iter()
        .filter(|(_, f)| f.name == "Doc.describe")
        .map(|(_, f)| f.symbol.as_str())
        .collect();
    assert_eq!(describe_symbols.len(), 2);
    // Each overload gets its own vtable slot (keyed by signature),
    // referencing the final persistent symbol by id.
    let doc_def = &module.classes[class_index(0)];
    assert_eq!(doc_def.vtable.len(), 2);
    for slot in &doc_def.vtable {
        let target = slot_fn(&module, slot);
        assert!(
            describe_symbols.contains(target),
            "vtable target {target} not among {describe_symbols:?}"
        );
    }
    let slot_symbols: std::collections::HashSet<&str> = doc_def
        .vtable
        .iter()
        .map(|slot| slot_fn(&module, slot))
        .collect();
    assert_eq!(slot_symbols.len(), 2);
}
