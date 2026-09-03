use super::*;

#[test]
fn exported_defaults_carry_kind_typed_references_and_access_witnesses() {
    let consume = with_default(
        fun_sig(
            "consume",
            vec![],
            vec![("token", ty_named("Token"))],
            Some(ty_named("Int")),
            vec![ret(Some(field(var("token"), "value")))],
        ),
        0,
        struct_init("Token", vec![call("produce", vec![])]),
    );
    let output = lower_user_output(file(vec![
        struct_decl("Token", vec![("value", ty_named("Int"))]),
        fun_expr("produce", vec![], vec![], Some(ty_named("Int")), int_lit(7)),
        consume,
        fun("main", vec![stmt(call("consume", vec![]))]),
    ]))
    .expect("an exported default must normalize every direct dependency");

    let (consume, _) = output
        .export
        .functions
        .iter()
        .find(|(_, function)| function.name == "consume")
        .expect("consume function");
    let interface = output
        .export
        .source_parameter_interfaces
        .iter()
        .find(|interface| interface.owner == hir::ExportParameterOwner::Function(consume))
        .expect("consume source interface");
    let hir::ExportParameterCalling::Default { source, .. } = interface.parameters[0].calling
    else {
        unreachable!()
    };
    let template = &output.export.export_default_exprs
        [output.export.export_default_sources[source].expression];
    assert_eq!(template.references.callables.len(), 1);
    assert_eq!(template.references.constructors.len(), 1);
    assert!(!template.references.types.is_empty());
    let expected_owner = hir::ExportParameterOwner::Function(consume);
    assert!(template.references.callables.iter().all(|reference| {
        reference.witness.owner == expected_owner
            && reference.witness.coverage == hir::ExportDefaultAccessCoverage::ConeWide
    }));
    assert!(template.references.constructors.iter().all(|reference| {
        reference.witness.owner == expected_owner
            && reference.witness.coverage == hir::ExportDefaultAccessCoverage::ConeWide
    }));
    assert!(
        template
            .references
            .types
            .iter()
            .all(|reference| reference.witness.owner == expected_owner)
    );
}

#[test]
fn override_rejects_new_defaults_vararg_mismatch_and_conflicting_sources() {
    let explicit_default = method_with_default(
        override_method_expr(
            "draw",
            vec![("width", ty_named("Int"))],
            Some(ty_named("Int")),
            var("width"),
        ),
        0,
        int_lit(20),
    );
    let errors = lower_user(file(vec![
        interface_decl(
            "Drawable",
            vec![method_with_default(
                bodyless_method(
                    false,
                    "draw",
                    vec![("width", ty_named("Int"))],
                    Some(ty_named("Int")),
                ),
                0,
                int_lit(10),
            )],
        ),
        class_decl(
            ast::ClassModifier::Final,
            "Shape",
            vec![],
            None,
            vec!["Drawable"],
            vec![explicit_default],
        ),
        fun("main", vec![]),
    ]))
    .expect_err("overrides cannot replace a default expression");
    assert!(errors.iter().any(|diagnostic| {
        diagnostic.message == "override function `draw` cannot declare a new default expression"
    }));

    let mismatch = override_method_expr(
        "add",
        vec![("values", ty_generic("Array", vec![ty_named("Int")]))],
        None,
        unit_lit(),
    );
    let errors = lower_user(file(vec![
        interface_decl(
            "Collector",
            vec![method_with_vararg(
                bodyless_method(false, "add", vec![("values", ty_named("Int"))], None),
                0,
            )],
        ),
        class_decl(
            ast::ClassModifier::Final,
            "CollectorImpl",
            vec![],
            None,
            vec!["Collector"],
            vec![mismatch],
        ),
        fun("main", vec![]),
    ]))
    .expect_err("an override must preserve the vararg source shape");
    assert!(errors.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("must have the same `vararg` shape")
    }));

    let interface = |name: &str, value| {
        interface_decl(
            name,
            vec![method_with_default(
                bodyless_method(
                    false,
                    "value",
                    vec![("input", ty_named("Int"))],
                    Some(ty_named("Int")),
                ),
                0,
                int_lit(value),
            )],
        )
    };
    let errors = lower_user(file(vec![
        interface("Left", 1),
        interface("Right", 2),
        class_decl(
            ast::ClassModifier::Final,
            "Both",
            vec![],
            None,
            vec!["Left", "Right"],
            vec![override_method_expr(
                "value",
                vec![("input", ty_named("Int"))],
                Some(ty_named("Int")),
                var("input"),
            )],
        ),
        fun("main", vec![]),
    ]))
    .expect_err("unrelated inherited default sources are ambiguous");
    assert!(errors.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("inherits conflicting default expressions")
    }));
}
