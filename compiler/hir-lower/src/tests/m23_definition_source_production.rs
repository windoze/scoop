use scoop_wire::{BudgetMeter, DecodeLimits, WirePath};

use super::*;

fn public_declarations(declarations: Vec<Decl>) -> Vec<Decl> {
    let mut source = file(declarations);
    make_core_public(&mut source);
    source.declarations
}

fn type_alias(name: &str, target: TypeRef) -> Decl {
    Decl::TypeAlias(ast::TypeAliasDecl {
        visibility: ast::VisibilitySyntax::Explicit {
            visibility: ast::DeclaredVisibility::Public,
            span: sp(),
        },
        name: ident(name),
        target,
        span: sp(),
    })
}

fn const_property(name: &str) -> Decl {
    Decl::Global(ast::PropertyDecl {
        annotations: Vec::new(),
        visibility: ast::VisibilitySyntax::Explicit {
            visibility: ast::DeclaredVisibility::Public,
            span: sp(),
        },
        modifier: ast::MethodModifier::Final,
        is_override: false,
        mutable: false,
        receiver_ty: None,
        type_params: Vec::new(),
        where_clause: None,
        name: ident(name),
        ty: ty_named("Int"),
        body: ast::PropertyBodySyntax::Const(Box::new(int_lit(7))),
        span: sp(),
    })
}

fn function_with_default() -> Decl {
    let Decl::Function(mut declaration) = fun_expr(
        "definitionSourceDefault",
        Vec::new(),
        vec![("first", ty_named("Int")), ("second", ty_named("Int"))],
        Some(ty_named("Int")),
        var("second"),
    ) else {
        unreachable!()
    };
    declaration.params[1].syntax = ast::ParameterSyntax::Default {
        expression: int_lit(11),
        equals_span: sp(),
    };
    Decl::Function(declaration)
}

#[test]
fn producer_collects_the_exact_deduplicated_definition_source_closure() {
    let module = lower_core_with_additional_declarations(public_declarations(vec![
        type_alias("DefinitionSourceAlias", ty_named("Int")),
        const_property("DefinitionSourceConstant"),
        function_with_default(),
    ]));
    let nominal_interfaces = hir::CanonicalNominalInterfacesV1::from_export_hir(&module).unwrap();
    let callable_interfaces = hir::CanonicalCallableInterfacesV1::from_export_hir(&module).unwrap();
    let property_interfaces = hir::CanonicalPropertyInterfacesV1::from_export_hir(&module).unwrap();
    let type_aliases = hir::CanonicalTypeAliasInterfacesV1::from_export_hir(&module).unwrap();
    let source_interfaces =
        hir::CanonicalCallableSourceInterfacesV1::from_export_hir(&module).unwrap();
    let default_templates =
        hir::CanonicalExportDefaultTemplatesV1::from_export_hir(&module).unwrap();
    let constants = hir::CanonicalExportConstValuesV1::from_export_hir(&module).unwrap();
    let definition_sources = hir::CanonicalExportDefinitionSourcesV1::from_interface_parts(
        &type_aliases,
        &source_interfaces,
        &default_templates,
        &constants,
    )
    .unwrap();

    assert!(!definition_sources.is_empty());
    assert!(
        definition_sources
            .sources()
            .windows(2)
            .all(|pair| pair[0] < pair[1])
    );

    let section = hir::CrossConeHirInterfaceSectionV1::new(
        module.public_export_bindings.clone(),
        nominal_interfaces,
        callable_interfaces,
        property_interfaces,
        type_aliases,
        source_interfaces,
        default_templates,
        constants,
        definition_sources,
        hir::CanonicalExternalHirReferencesV1::try_new(Vec::new()).unwrap(),
    );
    section
        .validate_definition_source_closure(
            &mut BudgetMeter::new(DecodeLimits::default()),
            &WirePath::root(),
        )
        .expect("the producer must emit every inline origin exactly once");
}
