use super::*;

#[test]
fn no_gc_value_type_contract_is_checked_in_hir_after_specialization() {
    let good = annotate_struct(
        struct_decl(
            "NativePair",
            vec![("x", ty_named("Int")), ("y", ty_named("UInt"))],
        ),
        vec![marker("NoGC")],
    );
    let good_enum = annotate_enum(
        enum_decl(
            "NativeResult",
            vec![],
            vec![
                variant_positional("Value", vec![ty_named("Int")]),
                variant_unit("Empty"),
            ],
        ),
        vec![marker("NoGC")],
    );
    let output = lower_user_output(file(vec![
        good,
        good_enum,
        fun_sig(
            "consumeResult",
            vec![],
            vec![("result", ty_named("NativeResult"))],
            None,
            vec![],
        ),
        fun("main", vec![]),
    ]))
    .expect("a concrete GC-free struct satisfies @NoGC");
    let (_, native_pair) = output
        .local
        .structs
        .iter()
        .find(|(_, declaration)| declaration.name == "NativePair")
        .expect("NativePair concrete definition");
    assert!(native_pair.gc_free);
    let (_, native_result) = output
        .local
        .enums
        .iter()
        .find(|(_, declaration)| declaration.name == "NativeResult")
        .expect("NativeResult concrete definition");
    assert!(native_result.gc_free);
    assert!(native_result.variants.iter().all(|variant| variant.gc_free));

    let bad_struct = annotate_struct(
        struct_decl("BadNativeValue", vec![("text", ty_named("String"))]),
        vec![marker("NoGC")],
    );
    let bad_enum = annotate_enum(
        enum_decl(
            "BadNativeEnum",
            vec![],
            vec![
                variant_positional("Text", vec![ty_named("String")]),
                variant_unit("Empty"),
            ],
        ),
        vec![marker("NoGC")],
    );
    let errors = messages(vec![bad_struct, bad_enum, fun("main", vec![])]);
    assert!(errors.iter().any(|message| {
        message.contains("`@NoGC` struct specialization `BadNativeValue`")
            && message.contains("contains a ref type")
    }));
    assert!(errors.iter().any(|message| {
        message.contains("`@NoGC` enum specialization `BadNativeEnum`")
            && message.contains("contains a ref type")
    }));
}

#[test]
fn no_gc_generic_type_waits_for_concrete_arguments() {
    let generic = annotate_struct(
        generic_struct_decl("NativeBox", vec!["T"], vec![("value", ty_named("T"))]),
        vec![marker("NoGC")],
    );
    lower_user(file(vec![generic.clone(), fun("main", vec![])]))
        .expect("an uninstantiated generic has no GC-free boolean result");

    let errors = messages(vec![
        generic,
        fun_sig(
            "consume",
            vec![],
            vec![("box", ty_generic("NativeBox", vec![ty_named("String")]))],
            None,
            vec![],
        ),
        fun("main", vec![]),
    ]);
    assert!(errors.iter().any(|message| {
        message.contains("`@NoGC` struct specialization `NativeBox<String>`")
            && message.contains("contains a ref type")
    }));
}
