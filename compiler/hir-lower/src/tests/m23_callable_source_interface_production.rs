use scoop_identity::{CallableTemplateOrigin, SignatureTypeKey};

use super::*;

fn public_declarations(declarations: Vec<Decl>) -> Vec<Decl> {
    let mut source = file(declarations);
    make_core_public(&mut source);
    source.declarations
}

fn with_default(mut declaration: Decl, parameter: usize, expression: Expr) -> Decl {
    let Decl::Function(function) = &mut declaration else {
        panic!("the test helper accepts a function declaration")
    };
    function.params[parameter].syntax = ast::ParameterSyntax::Default {
        expression,
        equals_span: sp(),
    };
    declaration
}

fn with_vararg(mut declaration: Decl, expression: Option<Expr>) -> Decl {
    let Decl::Function(function) = &mut declaration else {
        panic!("the test helper accepts a function declaration")
    };
    function.params[0].syntax = ast::ParameterSyntax::Vararg {
        modifier_span: sp(),
        default: expression.map_or(ast::VarargDefaultSyntax::EmptyWhenOmitted, |expression| {
            ast::VarargDefaultSyntax::Expression {
                expression,
                equals_span: sp(),
            }
        }),
    };
    declaration
}

fn function_id(module: &hir::Module, name: &str) -> hir::FunctionId {
    module
        .functions
        .iter()
        .find_map(|(id, function)| (function.name == name).then_some(id))
        .unwrap_or_else(|| panic!("missing function `{name}`"))
}

fn callable_id(module: &hir::Module, function: hir::FunctionId) -> CallableTemplateOrigin {
    match &module.function_identities[function] {
        hir::HirFunctionIdentity::Source(hir::HirSourceFunctionIdentity::Plain(record)) => {
            CallableTemplateOrigin::Function(record.id())
        }
        hir::HirFunctionIdentity::Source(hir::HirSourceFunctionIdentity::Generic(record)) => {
            CallableTemplateOrigin::GenericFunction(record.id())
        }
        identity => panic!("expected source callable identity, found {identity:?}"),
    }
}

#[test]
fn producer_projects_required_default_and_both_vararg_protocols() {
    let defaulted = with_default(
        fun_expr(
            "projectedDefaults",
            vec!["T"],
            vec![("required", ty_named("T")), ("optional", ty_named("T"))],
            Some(ty_named("T")),
            var("optional"),
        ),
        1,
        var("required"),
    );
    let empty_vararg = with_vararg(
        fun_sig(
            "projectedEmptyVararg",
            vec!["T"],
            vec![("values", ty_named("T"))],
            None,
            Vec::new(),
        ),
        None,
    );
    let default_vararg = with_vararg(
        fun_sig(
            "projectedDefaultVararg",
            Vec::new(),
            vec![("values", ty_named("Int"))],
            None,
            Vec::new(),
        ),
        Some(array_lit(vec![int_lit(7)])),
    );
    let module = lower_core_with_additional_declarations(public_declarations(vec![
        defaulted,
        empty_vararg,
        default_vararg,
        generic_struct_decl(
            "ProjectedSourceBox",
            vec!["T"],
            vec![("value", ty_named("T"))],
        ),
        enum_decl(
            "ProjectedSourceChoice",
            vec!["T"],
            vec![
                variant_unit("None"),
                variant_constructor("Some", vec![("value", ty_named("T"), None)]),
            ],
        ),
    ]));

    let callables = hir::CanonicalCallableInterfacesV1::from_export_hir(&module).unwrap();
    let interfaces = hir::CanonicalCallableSourceInterfacesV1::from_export_hir(&module)
        .expect("the public source-call protocol must project canonically");
    let expected_count = callables
        .records()
        .iter()
        .filter(|record| !matches!(record.declaration(), CallableTemplateOrigin::Accessor(_)))
        .count();
    assert_eq!(interfaces.records().len(), expected_count);
    assert!(
        interfaces
            .records()
            .iter()
            .all(|record| !matches!(record.owner(), CallableTemplateOrigin::Accessor(_)))
    );

    let defaulted = callable_id(&module, function_id(&module, "projectedDefaults"));
    let parameters = interfaces.get(defaulted).unwrap().parameters().parameters();
    assert!(matches!(
        parameters[0].calling(),
        hir::CallableParameterCallingV1::Required
    ));
    assert_eq!(
        parameters[0].value_type(),
        &SignatureTypeKey::Binder { depth: 0, index: 0 }
    );
    assert!(matches!(
        parameters[1].calling(),
        hir::CallableParameterCallingV1::Default { template }
            if *template == hir::ExportDefaultTemplateKeyV1::new(defaulted, 1)
    ));
    assert_eq!(
        parameters[1].definition_origin().origin().source().cone(),
        module.cone
    );

    let empty_vararg = callable_id(&module, function_id(&module, "projectedEmptyVararg"));
    let parameter = &interfaces
        .get(empty_vararg)
        .unwrap()
        .parameters()
        .parameters()[0];
    assert!(matches!(
        parameter.calling(),
        hir::CallableParameterCallingV1::VarargEmpty {
            element_type: SignatureTypeKey::Binder { depth: 0, index: 0 }
        }
    ));

    let default_vararg = callable_id(&module, function_id(&module, "projectedDefaultVararg"));
    let parameter = &interfaces
        .get(default_vararg)
        .unwrap()
        .parameters()
        .parameters()[0];
    assert!(matches!(
        parameter.calling(),
        hir::CallableParameterCallingV1::VarargDefault { template, .. }
            if *template == hir::ExportDefaultTemplateKeyV1::new(default_vararg, 0)
    ));

    let structure = module
        .structs
        .iter()
        .find_map(|(id, declaration)| (declaration.name == "ProjectedSourceBox").then_some(id))
        .unwrap();
    let constructor = module.structs[structure].constructors[0];
    let constructor =
        CallableTemplateOrigin::Constructor(module.constructor_identities[constructor].id());
    assert_eq!(
        interfaces
            .get(constructor)
            .unwrap()
            .parameters()
            .parameters()[0]
            .value_type(),
        &SignatureTypeKey::Binder { depth: 0, index: 0 }
    );

    let enumeration = module
        .enums
        .iter()
        .find_map(|(id, declaration)| (declaration.name == "ProjectedSourceChoice").then_some(id))
        .unwrap();
    let unit_variant = hir::EnumVariantRef::checked(&module.enums, enumeration, 0).unwrap();
    let unit_variant = CallableTemplateOrigin::VariantConstructor(
        module.enum_member_identities[unit_variant].id(),
    );
    assert!(
        interfaces
            .get(unit_variant)
            .unwrap()
            .parameters()
            .is_empty()
    );
}

#[test]
fn producer_rejects_a_parameter_origin_with_the_wrong_provider() {
    let mut module = lower_core_with_additional_declarations(public_declarations(vec![fun_sig(
        "brokenSourceOrigin",
        Vec::new(),
        vec![("value", ty_named("Int"))],
        None,
        Vec::new(),
    )]))
    .into_module();
    let function = function_id(&module, "brokenSourceOrigin");
    let interface = module
        .source_parameter_interfaces
        .iter_mut()
        .find(|interface| interface.owner == hir::ExportParameterOwner::Function(function))
        .unwrap();
    interface.parameters[0].origin.provider = hir::IntrinsicProviderId::from_raw(u32::MAX);

    assert!(matches!(
        hir::CanonicalCallableSourceInterfacesV1::from_export_hir(&module),
        Err(hir::CallableSourceInterfaceProductionError::Parameters {
            error: hir::CallableSourceParameterProjectionError::DefinitionOrigin {
                source: hir::HirDefinitionSourceProjectionError::ProviderMismatch { .. },
                ..
            },
            ..
        })
    ));
}
