use super::*;

#[test]
fn kind_bounds_are_typed_on_all_generic_hir_declarations() {
    let function = with_kind(
        fun_expr(
            "identity",
            vec!["T"],
            vec![("value", ty_named("T"))],
            Some(ty_named("T")),
            var("value"),
        ),
        ast::TypeParamKindBound::Value,
    );
    let strukt = with_kind(
        generic_struct_decl("RefBox", vec!["T"], vec![("value", ty_named("T"))]),
        ast::TypeParamKindBound::Ref,
    );
    let enumeration = with_kind(
        enum_decl("Choice", vec!["T"], vec![variant_unit("None")]),
        ast::TypeParamKindBound::Value,
    );
    let interface = with_kind(
        generic_interface_decl(
            "Source",
            vec!["T"],
            vec![bodyless_method(false, "get", vec![], Some(ty_named("T")))],
        ),
        ast::TypeParamKindBound::Ref,
    );
    let module = lower_user(file(vec![
        function,
        strukt,
        enumeration,
        interface,
        fun("main", vec![]),
    ]))
    .expect("kind bounds must lower to typed HIR");

    let identity = module
        .functions
        .iter()
        .find(|(_, function)| function.name == "identity")
        .unwrap()
        .1;
    assert_eq!(identity.type_params()[0].kind(), hir::TypeParamKind::Value);
    assert_eq!(
        module
            .structs
            .iter()
            .find(|(_, decl)| decl.name == "RefBox")
            .unwrap()
            .1
            .type_params[0]
            .kind(),
        hir::TypeParamKind::Ref
    );
    assert_eq!(
        module
            .enums
            .iter()
            .find(|(_, decl)| decl.name == "Choice")
            .unwrap()
            .1
            .type_params[0]
            .kind(),
        hir::TypeParamKind::Value
    );
    assert_eq!(
        module
            .interfaces
            .iter()
            .find(|(_, decl)| decl.name == "Source")
            .unwrap()
            .1
            .type_params[0]
            .kind(),
        hir::TypeParamKind::Ref
    );
    let dump = hir::dump(&module);
    assert!(dump.contains("struct RefBox<T : ref>"));
    assert!(dump.contains("enum Choice<T : value>"));
    assert!(dump.contains("interface Source<T : ref>"));
    assert!(dump.contains("fun identity<T : value>"));
}

#[test]
fn kind_bounds_check_concrete_function_and_struct_instantiations() {
    let value_identity = with_kind(
        fun_expr(
            "valueIdentity",
            vec!["T"],
            vec![("value", ty_named("T"))],
            Some(ty_named("T")),
            var("value"),
        ),
        ast::TypeParamKindBound::Value,
    );
    let ref_identity = with_kind(
        fun_expr(
            "refIdentity",
            vec!["T"],
            vec![("value", ty_named("T"))],
            Some(ty_named("T")),
            var("value"),
        ),
        ast::TypeParamKindBound::Ref,
    );
    let ref_box = with_kind(
        generic_struct_decl("RefBox", vec!["T"], vec![("value", ty_named("T"))]),
        ast::TypeParamKindBound::Ref,
    );
    let wrapper = struct_decl("Wrapper", vec![("text", ty_named("String"))]);

    lower_user(file(vec![
        value_identity.clone(),
        ref_identity.clone(),
        ref_box.clone(),
        wrapper.clone(),
        fun(
            "main",
            vec![
                val(
                    "value",
                    call(
                        "valueIdentity",
                        vec![struct_init("Wrapper", vec![str_lit("managed field")])],
                    ),
                ),
                val("reference", call("refIdentity", vec![str_lit("text")])),
                val("boxed", struct_init("RefBox", vec![str_lit("text")])),
            ],
        ),
    ]))
    .expect("value/ref kinds are independent from the recursive GC-free property");

    let errors = messages(vec![
        value_identity,
        ref_identity,
        ref_box,
        wrapper,
        fun(
            "main",
            vec![
                val("badValue", call("valueIdentity", vec![str_lit("text")])),
                val("badRef", call("refIdentity", vec![int_lit(1)])),
                val("badBox", struct_init("RefBox", vec![int_lit(1)])),
            ],
        ),
    ]);
    assert!(errors.iter().any(|message| {
        message.contains("fun valueIdentity<T : value>(value: T): T")
            && message.contains("type argument `String` for `T` must satisfy `value`")
    }));
    assert!(errors.iter().any(|message| {
        message.contains("fun refIdentity<T : ref>(value: T): T")
            && message.contains("type argument `Int` for `T` must satisfy `ref`")
    }));
    assert!(errors.iter().any(|message| {
        message.contains("struct RefBox<T : ref>(value: T)")
            && message.contains("type argument `Int` for `T` must satisfy `ref`")
    }));
}

#[test]
fn explicit_type_arguments_bind_functions_constructors_variants_and_method_suffixes() {
    let identity = fun_expr(
        "identity",
        vec!["T"],
        vec![("value", ty_named("T"))],
        Some(ty_named("T")),
        var("value"),
    );
    let box_decl = generic_struct_decl("Box", vec!["T"], vec![("value", ty_named("T"))]);
    let choice = enum_decl(
        "Choice",
        vec!["T"],
        vec![variant_positional("Some", vec![ty_named("T")])],
    );
    let mut convert = method_expr(
        "convert",
        vec![("value", ty_named("U"))],
        Some(ty_named("U")),
        var("value"),
    );
    convert.type_params = vec![type_param("U")];
    let holder = generic_struct_decl_full(
        "Holder",
        vec!["T"],
        vec![("value", ty_named("T"))],
        vec![],
        vec![convert],
    );
    let module = lower_user(file(vec![
        identity,
        box_decl,
        choice,
        holder,
        fun(
            "main",
            vec![
                val(
                    "function",
                    typed_call("identity", vec![ty_named("Int")], vec![int_lit(1)]),
                ),
                val(
                    "constructor",
                    typed_call("Box", vec![ty_named("String")], vec![str_lit("box")]),
                ),
                val(
                    "variant",
                    typed_method_call(
                        var("Choice"),
                        "Some",
                        vec![ty_named("Int")],
                        vec![int_lit(2)],
                    ),
                ),
                val("holder", struct_init("Holder", vec![int_lit(3)])),
                val(
                    "method",
                    typed_method_call(
                        var("holder"),
                        "convert",
                        vec![ty_named("String")],
                        vec![str_lit("method")],
                    ),
                ),
            ],
        ),
    ]))
    .expect("complete explicit type argument lists must lower");

    let dump = hir::dump(&module);
    assert!(dump.contains("Call identity<Int> : Int"), "{dump}");
    assert!(dump.contains("StructInit Box : Box<String>"), "{dump}");
    assert!(
        dump.contains("VariantConstruct Choice.Some<Int> : Choice<Int>"),
        "{dump}"
    );
    assert!(
        dump.contains("MethodCall Holder.convert : String")
            && dump.contains("method instance Holder.convert<owner=[Int], method=[String]>"),
        "{dump}"
    );
}

#[test]
fn explicit_type_arguments_filter_overloads_and_report_complete_list_errors() {
    let generic = fun_expr(
        "pick",
        vec!["T"],
        vec![("value", ty_named("T"))],
        Some(ty_named("T")),
        var("value"),
    );
    let concrete = fun_expr(
        "pick",
        vec![],
        vec![("value", ty_named("Int"))],
        Some(ty_named("Int")),
        var("value"),
    );
    let module = lower_user(file(vec![
        generic.clone(),
        concrete.clone(),
        fun(
            "main",
            vec![val(
                "picked",
                typed_call("pick", vec![ty_named("String")], vec![str_lit("value")]),
            )],
        ),
    ]))
    .expect("explicit arguments must exclude overloads with another generic arity");
    assert!(hir::dump(&module).contains("Call pick<String> : String"));

    let errors = messages(vec![
        generic,
        concrete,
        fun(
            "main",
            vec![stmt(typed_call(
                "pick",
                vec![ty_named("Int"), ty_named("String")],
                vec![int_lit(1)],
            ))],
        ),
    ]);
    assert_eq!(errors.len(), 1);
    assert!(errors[0].contains("fun pick<T>(value: T): T — expects 1 explicit type argument(s)"));
    assert!(errors[0].contains("fun pick(value: Int): Int — expects 0 explicit type argument(s)"));
}

#[test]
fn unconstrained_type_parameters_do_not_satisfy_a_stronger_kind() {
    let ref_box = with_kind(
        generic_struct_decl("RefBox", vec!["T"], vec![("value", ty_named("T"))]),
        ast::TypeParamKindBound::Ref,
    );
    let bad = fun_sig(
        "bad",
        vec!["U"],
        vec![("box", ty_generic("RefBox", vec![ty_named("U")]))],
        None,
        vec![],
    );
    let errors = messages(vec![ref_box.clone(), bad, fun("main", vec![])]);
    assert!(errors.iter().any(
        |message| message == "type argument `U` for `T` of struct `RefBox` must satisfy `ref`"
    ));

    let good = with_kind(
        fun_sig(
            "good",
            vec!["U"],
            vec![("box", ty_generic("RefBox", vec![ty_named("U")]))],
            None,
            vec![],
        ),
        ast::TypeParamKindBound::Ref,
    );
    lower_user(file(vec![ref_box, good, fun("main", vec![])]))
        .expect("a matching outer kind bound proves the nested application");
}

#[test]
fn ref_bound_parameters_have_reference_semantics_inside_the_definition() {
    let same = with_kind(
        fun_expr(
            "same",
            vec!["T"],
            vec![("left", ty_named("T")), ("right", ty_named("T"))],
            Some(ty_named("Boolean")),
            binary(ast::BinOp::RefEq, var("left"), var("right")),
        ),
        ast::TypeParamKindBound::Ref,
    );
    lower_user(file(vec![
        same,
        fun(
            "main",
            vec![val(
                "result",
                call("same", vec![str_lit("left"), str_lit("right")]),
            )],
        ),
    ]))
    .expect("a `T : ref` parameter must support reference-only operations");
}

#[test]
fn overloaded_kind_failure_keeps_the_precise_bound_diagnostic() {
    let constrained = with_kind(
        fun_expr(
            "choose",
            vec!["T"],
            vec![("value", ty_named("T"))],
            Some(ty_named("T")),
            var("value"),
        ),
        ast::TypeParamKindBound::Ref,
    );
    let other = fun_expr(
        "choose",
        vec![],
        vec![("left", ty_named("Int")), ("right", ty_named("Int"))],
        Some(ty_named("Int")),
        var("left"),
    );
    let errors = messages(vec![
        constrained,
        other,
        fun(
            "main",
            vec![val("result", call("choose", vec![int_lit(1)]))],
        ),
    ]);
    assert_eq!(errors.len(), 1);
    assert!(errors[0].contains(
        "fun choose<T : ref>(value: T): T — type argument `Int` for `T` must satisfy `ref`"
    ));
    assert!(errors[0].contains("fun choose(left: Int, right: Int): Int — expects 2 argument(s)"));
}
