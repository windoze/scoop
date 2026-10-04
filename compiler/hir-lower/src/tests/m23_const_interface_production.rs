use scoop_identity::DefinitionOriginSubject;

use super::*;

fn const_property(name: &str, ty: TypeRef, expression: Expr, public: bool) -> Decl {
    Decl::Global(ast::PropertyDecl {
        context_parameters: Vec::new(),
        annotations: Vec::new(),
        visibility: if public {
            ast::VisibilitySyntax::Explicit {
                visibility: ast::DeclaredVisibility::Public,
                span: sp(),
            }
        } else {
            ast::VisibilitySyntax::Omitted
        },
        modifier: ast::MethodModifier::Final,
        is_override: false,
        mutable: false,
        receiver_ty: None,
        type_params: Vec::new(),
        where_clause: None,
        name: ident(name),
        ty,
        body: ast::PropertyBodySyntax::Const(Box::new(expression)),
        span: sp(),
    })
}

fn runtime_property(name: &str) -> Decl {
    let Decl::Global(mut declaration) = const_property(name, ty_named("Int"), int_lit(7), true)
    else {
        unreachable!()
    };
    declaration.annotations.push(ast::Annotation {
        name: ident("Global"),
        args: Vec::new(),
        span: sp(),
    });
    declaration.mutable = true;
    declaration.body = ast::PropertyBodySyntax::Initializer {
        expression: Box::new(int_lit(7)),
        accessors: ast::AccessorSyntax::default(),
    };
    Decl::Global(declaration)
}

fn lowered_property(module: &hir::Module, name: &str) -> hir::PropertyId {
    module
        .properties
        .iter()
        .find_map(|(id, property)| (property.name == name).then_some(id))
        .unwrap_or_else(|| panic!("missing lowered property `{name}`"))
}

fn persistent_property(
    module: &hir::Module,
    property: hir::PropertyId,
) -> scoop_identity::PersistentPropertyId {
    module.property_identities[property]
        .ordinary_id()
        .expect("test const properties have ordinary identities")
}

#[test]
fn producer_projects_exactly_public_const_values_types_and_origins() {
    let module = lower_core_with_additional_declarations(vec![
        const_property(
            "ExportedIntegerConstant",
            ty_named("Int"),
            int_lit(42),
            true,
        ),
        const_property(
            "ExportedBooleanConstant",
            ty_named("Boolean"),
            bool_lit(true),
            true,
        ),
        const_property(
            "ExportedStringConstant",
            ty_named("String"),
            str_lit("forty-two"),
            true,
        ),
        const_property("PrivateConstant", ty_named("Int"), int_lit(11), false),
        runtime_property("ExportedRuntimeProperty"),
    ]);
    let values = hir::CanonicalExportConstValuesV1::from_export_hir(&module)
        .expect("the public const surface must project canonically");
    let mapper = hir::HirSignatureTypeMapper::new(hir::HirTypeIdentityInputs::from_export(&module));

    let expected = [
        (
            "ExportedIntegerConstant",
            hir::CanonicalConstValueV1::Integer(hir::CanonicalIntegerConstantV1::Signed32(42)),
        ),
        (
            "ExportedBooleanConstant",
            hir::CanonicalConstValueV1::Boolean(hir::CanonicalBooleanV1::True),
        ),
        (
            "ExportedStringConstant",
            hir::CanonicalConstValueV1::String("forty-two".to_string()),
        ),
    ];
    for (name, expected_value) in expected {
        let property = lowered_property(&module, name);
        let persistent = persistent_property(&module, property);
        let record = values
            .get(persistent)
            .unwrap_or_else(|| panic!("missing exported const `{name}`"));
        assert_eq!(record.value(), &expected_value);
        assert_eq!(
            record.value_type(),
            &mapper.map(module.properties[property].ty, &[]).unwrap()
        );
        assert_eq!(
            record.definition_origin().origin(),
            module
                .export_definition_origins
                .get(DefinitionOriginSubject::Property(persistent))
                .unwrap()
                .origin()
        );
    }

    for name in ["PrivateConstant", "ExportedRuntimeProperty"] {
        let property = lowered_property(&module, name);
        assert!(values.get(persistent_property(&module, property)).is_none());
    }
}

#[test]
fn producer_rejects_a_const_value_whose_hir_type_has_drifted() {
    let mut module = lower_core_with_additional_declarations(vec![const_property(
        "MismatchedConstant",
        ty_named("Int"),
        int_lit(42),
        true,
    )])
    .into_module();
    let property = lowered_property(&module, "MismatchedConstant");
    let persistent = persistent_property(&module, property);
    module.properties[property].ty = module.boolean;

    assert_eq!(
        hir::CanonicalExportConstValuesV1::from_export_hir(&module),
        Err(hir::ExportConstValueBuildError::ValueTypeMismatch {
            property: persistent,
            value: hir::CanonicalConstValueKindV1::Integer(hir::IntegerKind::SIGNED_32),
            actual_type: Some(hir::CanonicalConstValueKindV1::Boolean),
        })
    );
}
