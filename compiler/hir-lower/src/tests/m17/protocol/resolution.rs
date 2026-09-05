use super::*;

#[test]
fn msc_prefers_fewer_defaults_then_a_non_vararg_declaration() {
    let defaulted = with_default(
        fun_expr(
            "pick",
            vec![],
            vec![("x", ty_named("Int")), ("y", ty_named("Int"))],
            Some(ty_named("String")),
            str_lit("defaulted"),
        ),
        1,
        int_lit(0),
    );
    let vararg = with_vararg(
        fun_expr(
            "gather",
            vec![],
            vec![("values", ty_named("Int"))],
            Some(ty_named("String")),
            str_lit("vararg"),
        ),
        0,
    );
    let module = lower_user(file(vec![
        fun_expr(
            "pick",
            vec![],
            vec![("x", ty_named("Int"))],
            Some(ty_named("Int")),
            int_lit(1),
        ),
        defaulted,
        fun_expr(
            "gather",
            vec![],
            vec![("value", ty_named("Int"))],
            Some(ty_named("Int")),
            int_lit(2),
        ),
        vararg,
        fun(
            "main",
            vec![
                val("a", call("pick", vec![int_lit(1)])),
                val("b", call("gather", vec![int_lit(1)])),
            ],
        ),
    ]))
    .expect("MSC additions choose a unique candidate");
    let main = function_body(&module, "main");
    let result_types = main
        .locals
        .iter()
        .filter(|(_, local)| local.name == "a" || local.name == "b")
        .map(|(_, local)| hir::type_name(&module, local.ty))
        .collect::<Vec<_>>();
    assert_eq!(result_types, vec!["Int", "Int"]);
}

#[test]
fn override_inherits_default_but_each_static_view_keeps_its_parameter_name() {
    let interface_method = method_with_default(
        bodyless_method(
            false,
            "draw",
            vec![("width", ty_named("Int"))],
            Some(ty_named("Int")),
        ),
        0,
        int_lit(10),
    );
    let implementation = override_method_expr(
        "draw",
        vec![("size", ty_named("Int"))],
        Some(ty_named("Int")),
        var("size"),
    );
    let module = lower_user(file(vec![
        interface_decl("Drawable", vec![interface_method]),
        class_decl(
            ast::ClassModifier::Final,
            "Shape",
            vec![],
            None,
            vec!["Drawable"],
            vec![implementation],
        ),
        fun(
            "main",
            vec![
                val("shape", call("Shape", vec![])),
                val("defaulted", method_call(var("shape"), "draw", vec![])),
                val(
                    "concreteName",
                    source_method_call(
                        var("shape"),
                        "draw",
                        vec![named_argument("size", int_lit(1))],
                    ),
                ),
                val_ty(
                    "drawable",
                    Some(ty_named("Drawable")),
                    call("Shape", vec![]),
                ),
                val(
                    "interfaceName",
                    source_method_call(
                        var("drawable"),
                        "draw",
                        vec![named_argument("width", int_lit(2))],
                    ),
                ),
            ],
        ),
    ]))
    .expect("an override inherits one default source while keeping its own parameter name");
    let dump = hir::dump(&module);
    assert!(dump.contains("IntegerLiteral 10 : Int"), "{dump}");
    assert!(dump.contains("MethodCall Shape.draw"), "{dump}");
    assert!(dump.contains("MethodCall Drawable.draw"), "{dump}");
}

#[test]
fn inherited_default_carries_the_parent_to_child_type_relation() {
    let inherited = method_with_default(
        bodyless_method(
            false,
            "choose",
            vec![("seed", ty_named("P")), ("value", ty_named("P"))],
            Some(ty_named("P")),
        ),
        1,
        var("seed"),
    );
    let implementation = override_method_expr(
        "choose",
        vec![
            ("seed", ty_generic("Array", vec![ty_named("T")])),
            ("value", ty_generic("Array", vec![ty_named("T")])),
        ],
        Some(ty_generic("Array", vec![ty_named("T")])),
        var("value"),
    );
    let mut child_declaration = class_decl(
        ast::ClassModifier::Final,
        "Child",
        vec![],
        None,
        vec![],
        vec![implementation],
    );
    let Decl::Class(child) = &mut child_declaration else {
        unreachable!()
    };
    child.type_params = vec![type_param("T")];
    child.supertypes = vec![bare_supertype(ty_generic(
        "Parent",
        vec![ty_generic("Array", vec![ty_named("T")])],
    ))];

    let output = lower_user_output(file(vec![
        generic_interface_decl("Parent", vec!["P"], vec![inherited]),
        child_declaration,
        fun(
            "main",
            vec![
                val("child", typed_call("Child", vec![ty_named("Int")], vec![])),
                val(
                    "selected",
                    method_call(var("child"), "choose", vec![array_lit(vec![int_lit(1)])]),
                ),
            ],
        ),
    ]))
    .expect("the inherited template parameter must map through Array<T>");

    let main = function_body(&output.export, "main");
    let selected = main
        .locals
        .iter()
        .find(|(_, local)| local.name == "selected")
        .map(|(_, local)| local)
        .expect("selected local");
    assert_eq!(hir::type_name(&output.export, selected.ty), "Array<Int>");
}
