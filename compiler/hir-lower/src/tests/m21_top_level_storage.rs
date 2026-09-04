use super::*;

fn top_level_property(
    name: &str,
    mutable: bool,
    ty: TypeRef,
    body: ast::PropertyBodySyntax,
) -> ast::PropertyDecl {
    ast::PropertyDecl {
        annotations: Vec::new(),
        visibility: ast::VisibilitySyntax::Omitted,
        modifier: ast::MethodModifier::Final,
        is_override: false,
        mutable,
        receiver_ty: None,
        type_params: Vec::new(),
        where_clause: None,
        name: ident(name),
        ty,
        body,
        span: sp(),
    }
}

fn initializer(expression: Expr, accessors: ast::AccessorSyntax) -> ast::PropertyBodySyntax {
    ast::PropertyBodySyntax::Initializer {
        expression: Box::new(expression),
        accessors,
    }
}

#[test]
fn ordinary_top_level_properties_own_managed_storage_and_backing_accessors() {
    let seed = ast::PropertyDecl {
        body: ast::PropertyBodySyntax::Const(Box::new(int_lit(40))),
        ..top_level_property(
            "seed",
            false,
            ty_named("Int"),
            initializer(int_lit(0), ast::AccessorSyntax::default()),
        )
    };
    let custom_accessors = ast::AccessorSyntax {
        getter: Some(ast::GetterDecl {
            annotations: Vec::new(),
            body: ast::AccessorBodySyntax::Expr(Box::new(binary(
                BinOp::Add,
                var("field"),
                int_lit(1),
            ))),
            span: sp(),
        }),
        setter: None,
    };
    let source = file(vec![
        Decl::Global(seed),
        Decl::Global(top_level_property(
            "answer",
            false,
            ty_named("Int"),
            initializer(
                binary(BinOp::Add, var("seed"), int_lit(2)),
                ast::AccessorSyntax::default(),
            ),
        )),
        Decl::Global(top_level_property(
            "label",
            true,
            ty_named("String"),
            initializer(str_lit("ready"), ast::AccessorSyntax::default()),
        )),
        Decl::Global(top_level_property(
            "optional",
            true,
            ty_nullable(ty_named("String")),
            ast::PropertyBodySyntax::OptionalOmitted,
        )),
        Decl::Global(top_level_property(
            "adjusted",
            true,
            ty_named("Int"),
            initializer(int_lit(4), custom_accessors),
        )),
        fun("main", Vec::new()),
    ]);
    let module = lower_user(source).expect("static top-level properties must lower");

    assert_eq!(module.globals.len(), 4);
    for (global_id, global) in module.globals.iter() {
        assert!(matches!(global.storage, hir::GlobalStorage::Managed { .. }));
        let property = &module.properties[global.property];
        assert!(matches!(
            property.representation,
            hir::PropertyRepresentation::Stored(hir::StoredProperty {
                backing: hir::PropertyBacking::TopLevelGlobal { storage }
            }) if storage == global_id
        ));
    }

    let optional = module
        .globals
        .iter()
        .find_map(|(_, global)| (global.name == "optional").then_some(global))
        .expect("optional global");
    assert!(matches!(
        optional.storage,
        hir::GlobalStorage::Managed {
            initializer: hir::ConstantValue::EnumUnit { .. }
        }
    ));
    let label = module
        .globals
        .iter()
        .find_map(|(_, global)| (global.name == "label").then_some(global))
        .expect("label global");
    assert!(matches!(
        &label.storage,
        hir::GlobalStorage::Managed {
            initializer: hir::ConstantValue::String(value)
        } if value == "ready"
    ));

    let adjusted = module
        .properties
        .iter()
        .find_map(|(_, property)| (property.name == "adjusted").then_some(property))
        .expect("adjusted property");
    assert!(matches!(
        module.property_getters[adjusted.capability.getter()].implementation,
        hir::PropertyAccessorImplementation::Body(_)
    ));
}

#[test]
fn non_static_top_level_initializer_is_kept_out_of_image_storage() {
    let source = file(vec![
        Decl::Global(top_level_property(
            "value",
            false,
            ty_named("Int"),
            initializer(call("make", Vec::new()), ast::AccessorSyntax::default()),
        )),
        fun_expr(
            "make",
            Vec::new(),
            Vec::new(),
            Some(ty_named("Int")),
            int_lit(1),
        ),
        fun("main", Vec::new()),
    ]);
    let errors = lower_user(source).expect_err("a runtime initializer needs an init unit");
    assert!(errors.iter().any(|diagnostic| {
        diagnostic.message
            == "top-level property initializer is not statically representable and requires a runtime initialization unit"
    }));
}
