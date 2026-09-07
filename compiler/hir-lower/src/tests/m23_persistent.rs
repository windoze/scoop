//! Persistent identity tests over lowered modules (DESIGN 3.1).

use super::*;

use scoop_identity::{ConeCoordinate, ConeIdentity};

fn world() -> hir::PersistentWorld {
    hir::PersistentWorld::single_unit(
        ConeIdentity::of(&ConeCoordinate::reserved_core()),
        ConeIdentity::of(&ConeCoordinate::new("dev.test", "user", "0.1.0").unwrap()),
    )
}

fn other_user_world() -> hir::PersistentWorld {
    hir::PersistentWorld::single_unit(
        ConeIdentity::of(&ConeCoordinate::reserved_core()),
        ConeIdentity::of(&ConeCoordinate::new("dev.test", "other", "0.2.0").unwrap()),
    )
}

fn user_function(module: &hir::Module, name: &str) -> hir::FunctionId {
    module
        .functions
        .iter()
        .find(|(_, function)| {
            function.name == name
                && function.method.is_none()
                && matches!(function.kind, hir::FunctionKind::User(_))
        })
        .map(|(id, _)| id)
        .unwrap_or_else(|| panic!("function {name} exists"))
}

fn hello_file() -> SourceFile {
    file(vec![
        fun("main", vec![stmt(call("print", vec![str_lit("hello")]))]),
        fun("helper", vec![]),
    ])
}

#[test]
fn function_ids_are_stable_and_cone_scoped() {
    let module = lower_user(hello_file()).expect("lowers");
    let persistent_world = world();
    let mut ids = hir::PersistentIds::new(&module, &persistent_world);
    let main_id = ids.function_id(module.entry).expect("entry has an id");
    let helper_id = ids
        .function_id(user_function(&module, "helper"))
        .expect("helper has an id");
    assert_ne!(main_id.as_bytes(), helper_id.as_bytes());

    // Recomputation is deterministic.
    let again_world = world();
    let mut again = hir::PersistentIds::new(&module, &again_world);
    assert_eq!(again.function_id(module.entry), Some(main_id));

    // A different user Cone changes user-owned ids but not core ones.
    let other_persistent_world = other_user_world();
    let mut other = hir::PersistentIds::new(&module, &other_persistent_world);
    let other_main = other.function_id(module.entry).expect("id");
    assert_ne!(main_id.as_bytes(), other_main.as_bytes());

    // Core entities keep their ids across user cones.
    let option_enum = module.option_core.enumeration();
    let core_id = ids.enum_id(option_enum).expect("core enum id");
    let other_core_id = other.enum_id(option_enum).expect("core enum id");
    assert_eq!(core_id.as_bytes(), other_core_id.as_bytes());
}

#[test]
fn signature_key_separates_parameter_shapes() {
    let module = lower_user(file(vec![
        fun("main", vec![]),
        fun_sig(
            "g",
            vec![],
            vec![("x", ty_named("Int"))],
            None,
            vec![stmt(call("print", vec![int_lit(1)]))],
        ),
        fun_sig("g", vec![], vec![("x", ty_named("Boolean"))], None, vec![]),
    ]))
    .expect("overloads lower");
    let first = user_function(&module, "g");
    let persistent_world = world();
    let mut ids = hir::PersistentIds::new(&module, &persistent_world);
    let first_key = ids
        .function_signature_key(&module.functions[first])
        .expect("key");
    let second = module
        .functions
        .iter()
        .find(|(id, function)| function.name == "g" && *id != first && function.method.is_none())
        .map(|(id, _)| id)
        .expect("second overload");
    let second_key = ids
        .function_signature_key(&module.functions[second])
        .expect("key");
    assert_ne!(first_key, second_key);
    // Both overloads resolve to distinct persistent ids.
    assert_ne!(
        ids.function_id(first).unwrap().as_bytes(),
        ids.function_id(second).unwrap().as_bytes()
    );
}

#[test]
fn nominal_ids_distinguish_kinds_with_the_same_name() {
    let with_struct = lower_user(file(vec![fun("main", vec![]), struct_decl("P", vec![])]))
        .expect("struct lowers");
    let with_class = lower_user(file(vec![
        fun("main", vec![]),
        class_decl(ast::ClassModifier::Final, "P", vec![], None, vec![], vec![]),
    ]))
    .expect("class lowers");

    let struct_id = {
        let persistent_world = world();
        let mut ids = hir::PersistentIds::new(&with_struct, &persistent_world);
        let decl = with_struct
            .structs
            .iter()
            .find(|(_, decl)| decl.name == "P")
            .map(|(id, _)| id)
            .expect("struct P");
        ids.struct_id(decl).expect("id")
    };
    let class_id = {
        let persistent_world = world();
        let mut ids = hir::PersistentIds::new(&with_class, &persistent_world);
        let decl = with_class
            .classes
            .iter()
            .find(|(_, decl)| decl.name == "P")
            .map(|(id, _)| id)
            .expect("class P");
        ids.class_id(decl).expect("id")
    };
    assert_ne!(struct_id.as_bytes(), class_id.as_bytes());
}

#[test]
fn signature_key_separates_extension_from_plain_function() {
    // `fun f(x: Int)` and an extension `fun Int.f()` share the name and
    // the ABI parameter shape; the signature key's extension bit keeps
    // their ids distinct.
    let plain_module = lower_user(file(vec![
        fun("main", vec![]),
        fun_sig("f", vec![], vec![("x", ty_named("Int"))], None, vec![]),
    ]))
    .expect("plain lowers");
    let plain = user_function(&plain_module, "f");
    let persistent_world = world();
    let mut ids = hir::PersistentIds::new(&plain_module, &persistent_world);
    let plain_key = ids
        .function_signature_key(&plain_module.functions[plain])
        .expect("key");

    let extension = {
        let mut declaration = fun_sig("f", vec![], vec![], None, vec![]);
        if let ast::Decl::Function(function) = &mut declaration {
            function.receiver_ty = Some(ty_named("Int"));
        }
        declaration
    };
    let ext_module =
        lower_user(file(vec![fun("main", vec![]), extension])).expect("extension lowers");
    let ext = user_function(&ext_module, "f");
    let persistent_world = world();
    let mut ext_ids = hir::PersistentIds::new(&ext_module, &persistent_world);
    let ext_key = ext_ids
        .function_signature_key(&ext_module.functions[ext])
        .expect("key");
    assert_ne!(plain_key, ext_key);
}

#[test]
fn concrete_module_carries_a_complete_exact_type_table() {
    let output = lower_user_output(file(vec![
        fun("main", vec![stmt(call("work", vec![int_lit(7)]))]),
        fun_sig(
            "work",
            vec![],
            vec![("value", ty_named("Int"))],
            None,
            vec![stmt(call("print", vec![str_lit("w")]))],
        ),
    ]))
    .expect("lowers");
    let module = &output.local;

    // Every interned concrete type has an exact identity and the table
    // validates (ids recompute from keys; the reference graph is acyclic).
    assert_eq!(module.exact_of.len(), module.types.iter().count());
    module.exact_types.validate().expect("table validates");

    // Integer kinds and Unit stay distinct nominal identities.
    let int = module
        .types
        .iter()
        .find(|(_, ty)| {
            matches!(
                ty.kind,
                hir::concrete::TypeKind::Integer(hir::IntegerKind::SIGNED_32)
            )
        })
        .map(|(id, _)| id)
        .expect("Int type");
    let boolean = module
        .types
        .iter()
        .find(|(_, ty)| matches!(ty.kind, hir::concrete::TypeKind::Boolean))
        .map(|(id, _)| id)
        .expect("Boolean type");
    assert_ne!(module.exact_of[&int], module.exact_of[&boolean]);

    // Option<Int> from core remains reachable through the table.
    let option_int = module
        .types
        .iter()
        .find(|(_, ty)| {
            matches!(&ty.kind, hir::concrete::TypeKind::Enum(id) if module.enums[*id].name.starts_with("Option"))
        })
        .map(|(id, _)| id)
        .expect("Option instantiation exists");
    assert!(module.exact_of.contains_key(&option_int));

    // The same compilation under a different cone identity produces the
    // same structural exact ids for core types (they are core-scoped).
    let second = lower_user_output(file(vec![
        fun("main", vec![stmt(call("work", vec![int_lit(7)]))]),
        fun_sig(
            "work",
            vec![],
            vec![("value", ty_named("Int"))],
            None,
            vec![stmt(call("print", vec![str_lit("w")]))],
        ),
    ]))
    .expect("lowers again");
    assert_eq!(
        module.exact_of[&int],
        second.local.exact_of[&{
            second
                .local
                .types
                .iter()
                .find(|(_, ty)| {
                    matches!(
                        ty.kind,
                        hir::concrete::TypeKind::Integer(hir::IntegerKind::SIGNED_32)
                    )
                })
                .map(|(id, _)| id)
                .expect("Int type")
        }]
    );
}

#[test]
fn concrete_functions_carry_persistent_symbols() {
    let output = lower_user_output(file(vec![
        fun("main", vec![stmt(call("work", vec![int_lit(1)]))]),
        fun_sig(
            "work",
            vec![],
            vec![("value", ty_named("Int"))],
            None,
            vec![stmt(call("print", vec![str_lit("w")]))],
        ),
        fun_sig(
            "work",
            vec![],
            vec![("value", ty_named("Boolean"))],
            None,
            vec![],
        ),
    ]))
    .expect("lowers");
    let module = &output.local;
    let symbols: Vec<&str> = module
        .functions
        .iter()
        .map(|(_, function)| function.symbol.as_str())
        .collect();
    // User functions and core methods all carry mangled symbols.
    assert!(
        symbols
            .iter()
            .all(|symbol| { symbol.is_empty() || symbol.starts_with("scoop$1$") })
    );
    // The two `work` overloads have distinct symbols despite one name.
    let work_symbols: Vec<&str> = module
        .functions
        .iter()
        .filter(|(_, function)| function.name == "work")
        .map(|(_, function)| function.symbol.as_str())
        .collect();
    assert_eq!(work_symbols.len(), 2);
    assert_ne!(work_symbols[0], work_symbols[1]);
    assert!(work_symbols.iter().all(|s| s.starts_with("scoop$1$fn$")));

    // Intrinsics carry no Scoop symbol.
    assert!(module.functions.iter().any(|(_, function)| {
        matches!(function.kind, hir::concrete::FunctionKind::Intrinsic(_))
            && function.symbol.is_empty()
    }));
}

#[test]
fn nested_same_name_nominals_get_distinct_persistent_ids() {
    // Regression: nested nominal keys must carry the typed owner chain;
    // `First.Nested` and `Second.Nested` are distinct entities.
    let outer = |owner: &str| {
        let mut declaration = class_decl(
            ast::ClassModifier::Final,
            owner,
            vec![],
            None,
            vec![],
            vec![],
        );
        let Decl::Class(class) = &mut declaration else {
            unreachable!("class builder returns a class");
        };
        let Decl::Class(nested) = class_decl(
            ast::ClassModifier::Final,
            "Nested",
            vec![],
            None,
            vec![],
            vec![],
        ) else {
            unreachable!("class builder returns a class");
        };
        class.members.push(ast::ClassMember::Nested(Box::new(
            ast::NestedNominalDecl::Class(Box::new(nested)),
        )));
        declaration
    };
    let first = outer("First");
    let second = outer("Second");
    let output = lower_user_output(file(vec![first, second, fun("main", vec![])]))
        .expect("nested declarations lower");
    let persistent_world = world();
    let mut ids = hir::PersistentIds::new(&output.export, &persistent_world);
    let nested_ids: Vec<scoop_identity::persistent::PersistentTypeId> = output
        .export
        .classes
        .iter()
        .filter(|(_, declaration)| declaration.name == "Nested")
        .filter_map(|(id, _)| ids.class_id(id))
        .collect();
    assert_eq!(nested_ids.len(), 2, "two nested classes exist");
    assert_ne!(
        nested_ids[0].as_bytes(),
        nested_ids[1].as_bytes(),
        "same-named nested classes under distinct owners must not collide"
    );
}

#[test]
fn generic_template_ids_stay_distinct_across_nominal_kinds() {
    // Regression: generic template caches are per-kind; raw arena indices
    // collide across the struct/enum/class/interface arenas. One
    // declaration of each kind lands at raw index 0 in its own arena, so
    // a shared cache would return the first kind's template for every
    // later kind.
    let module = lower_user(file(vec![
        fun("main", vec![]),
        generic_struct_decl("Cell", vec!["T"], vec![]),
        class_decl(
            ast::ClassModifier::Final,
            "Node",
            vec![],
            None,
            vec![],
            vec![],
        ),
    ]))
    .expect("distinct-named declarations lower");
    let persistent_world = world();
    let mut ids = hir::PersistentIds::new(&module, &persistent_world);
    let struct_decl_id = module
        .structs
        .iter()
        .find(|(_, declaration)| declaration.name == "Cell")
        .map(|(id, _)| id)
        .expect("struct Cell");
    let class_decl_id = module
        .classes
        .iter()
        .find(|(_, declaration)| declaration.name == "Node")
        .map(|(id, _)| id)
        .expect("class Node");
    let struct_template = ids.generic_struct_id(struct_decl_id).unwrap();
    // Before the per-kind caches, a class lookup whose raw index happened
    // to equal an already-cached struct index returned the struct
    // template id. The caches are keyed per-kind now, so distinct
    // declarations never alias regardless of index overlap.
    let class_template = ids.generic_class_id(class_decl_id).unwrap();
    assert_ne!(struct_template.as_bytes(), class_template.as_bytes());
    // And the plain ids differ by kind tag at the same raw index.
    assert_ne!(
        ids.struct_id(struct_decl_id).unwrap().as_bytes(),
        ids.class_id(class_decl_id).unwrap().as_bytes()
    );
}
