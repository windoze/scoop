use super::*;

#[test]
fn exported_defaults_preserve_kind_typed_references_and_definition_locations() {
    let public = ast::VisibilitySyntax::Explicit {
        visibility: ast::DeclaredVisibility::Public,
        span: sp(),
    };
    let mut consume = with_default(
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
    let Decl::Function(consume_decl) = &mut consume else {
        unreachable!()
    };
    consume_decl.visibility = public;
    let mut token = struct_decl("Token", vec![("value", ty_named("Int"))]);
    let Decl::Struct(token_decl) = &mut token else {
        unreachable!()
    };
    token_decl.visibility = public;
    let mut produce = fun_expr("produce", vec![], vec![], Some(ty_named("Int")), int_lit(7));
    let Decl::Function(produce_decl) = &mut produce else {
        unreachable!()
    };
    produce_decl.visibility = public;
    let output = lower_user_output(file(vec![
        token,
        produce,
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
    assert_eq!(
        template.definition_root,
        hir::LexicalDefinitionRoot::Function(consume)
    );
    assert_eq!(
        definition_path(&template.definition_path),
        vec![(
            scoop_identity::StructuralDefinitionSiteRole::DefaultValue,
            0,
        )]
    );
    assert_eq!(template.references.callables.len(), 1);
    assert_eq!(template.references.constructors.len(), 1);
    assert!(!template.references.types.is_empty());
    assert!(matches!(
        template.references.callables[0].target,
        hir::ExportDefaultCallableTarget::Callable(_),
    ));
    assert!(matches!(
        template.references.constructors[0].target,
        hir::ExportDefaultConstructorTarget::Struct(_),
    ));
    assert_eq!(
        template.references.callables[0].origin.provider,
        template.origin.provider
    );
    assert_eq!(
        template.references.constructors[0].origin.provider,
        template.origin.provider
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
