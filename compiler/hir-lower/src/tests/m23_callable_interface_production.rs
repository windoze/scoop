use scoop_identity::{CallableTemplateOrigin, SignatureTypeKey};

use super::*;

fn public_declarations(declarations: Vec<Decl>) -> Vec<Decl> {
    let mut source = file(declarations);
    make_core_public(&mut source);
    source.declarations
}

fn mutable_property(name: &str) -> Decl {
    Decl::Global(ast::PropertyDecl {
        annotations: Vec::new(),
        visibility: ast::VisibilitySyntax::Omitted,
        modifier: ast::MethodModifier::Final,
        is_override: false,
        mutable: true,
        receiver_ty: None,
        type_params: Vec::new(),
        where_clause: None,
        name: ident(name),
        ty: ty_named("Int"),
        body: ast::PropertyBodySyntax::Computed(ast::AccessorSyntax {
            getter: Some(ast::GetterDecl {
                annotations: Vec::new(),
                body: ast::AccessorBodySyntax::Expr(Box::new(int_lit(4))),
                span: sp(),
            }),
            setter: Some(ast::SetterDecl {
                annotations: Vec::new(),
                visibility: ast::SetterVisibilitySyntax::Inherited,
                parameter: ast::SetterParameterSyntax::Default { span: sp() },
                body: ast::AccessorBodySyntax::Block(block(Vec::new())),
                span: sp(),
            }),
        }),
        span: sp(),
    })
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

fn function_id(module: &hir::Module, name: &str) -> hir::FunctionId {
    module
        .functions
        .iter()
        .find_map(|(id, declaration)| (declaration.name == name).then_some(id))
        .unwrap_or_else(|| panic!("missing function `{name}`"))
}

fn struct_id(module: &hir::Module, name: &str) -> hir::StructId {
    module
        .structs
        .iter()
        .find_map(|(id, declaration)| (declaration.name == name).then_some(id))
        .unwrap_or_else(|| panic!("missing struct `{name}`"))
}

fn enum_id(module: &hir::Module, name: &str) -> hir::EnumId {
    module
        .enums
        .iter()
        .find_map(|(id, declaration)| (declaration.name == name).then_some(id))
        .unwrap_or_else(|| panic!("missing enum `{name}`"))
}

#[test]
fn producer_projects_functions_constructors_variants_and_accessors() {
    let extension = extension_expr(
        ty_named("T"),
        "projectedEcho",
        vec!["T"],
        vec![("other", ty_named("T"))],
        Some(ty_named("T")),
        var("other"),
    );
    let structure = generic_struct_decl(
        "ProjectedCallableBox",
        vec!["T"],
        vec![("value", ty_named("T"))],
    );
    let enumeration = enum_decl(
        "ProjectedCallableChoice",
        vec!["T"],
        vec![
            variant_unit("None"),
            variant_constructor("Some", vec![("value", ty_named("T"), None)]),
        ],
    );
    let interface = interface_decl(
        "ProjectedCallableContract",
        vec![
            bodyless_method(true, "required", Vec::new(), Some(ty_named("Int"))),
            method_expr("provided", Vec::new(), Some(ty_named("Int")), int_lit(8)),
        ],
    );
    let module = lower_core_with_additional_declarations(public_declarations(vec![
        extension,
        structure,
        enumeration,
        interface,
        mutable_property("ProjectedMutable"),
    ]));

    let callables = hir::CanonicalCallableInterfacesV1::from_export_hir(&module)
        .expect("the complete public callable surface must project canonically");
    let source_class_constructors = module
        .public_surface
        .class_constructors
        .iter()
        .filter(|&&id| {
            matches!(
                module.constructor_identities[id],
                hir::HirClassConstructorIdentity::Source(_)
            )
        })
        .count();
    let variant_count = module
        .public_surface
        .enums
        .iter()
        .map(|&id| module.enums[id].variants.len())
        .sum::<usize>();
    assert_eq!(
        callables.records().len(),
        module.public_surface.functions.len()
            + module.public_surface.struct_constructors.len()
            + source_class_constructors
            + variant_count
            + module.public_surface.property_getters.len()
            + module.public_surface.property_setters.len()
    );

    let extension = function_id(&module, "projectedEcho");
    let extension = callables.get(callable_id(&module, extension)).unwrap();
    assert_eq!(extension.owner(), hir::PublicDeclarationOwnerV1::Extension);
    assert_eq!(extension.type_parameters().binders().len(), 1);
    assert_eq!(
        extension.receiver(),
        Some(&SignatureTypeKey::Binder { depth: 0, index: 0 })
    );
    assert_eq!(
        extension.parameters().parameters()[0].value_type(),
        &SignatureTypeKey::Binder { depth: 0, index: 0 }
    );
    assert_eq!(
        extension.result(),
        &SignatureTypeKey::Binder { depth: 0, index: 0 }
    );

    let structure = struct_id(&module, "ProjectedCallableBox");
    let constructor = module.structs[structure].constructors[0];
    let constructor_id = module.constructor_identities[constructor].id();
    let constructor = callables
        .get(CallableTemplateOrigin::Constructor(constructor_id))
        .unwrap();
    assert_eq!(
        constructor.parameters().parameters()[0].name().as_str(),
        "value"
    );
    assert_eq!(
        constructor.parameters().parameters()[0].value_type(),
        &SignatureTypeKey::Binder { depth: 0, index: 0 }
    );
    assert!(matches!(
        constructor.result(),
        SignatureTypeKey::NominalApplication { arguments, .. }
            if arguments.as_slice() == [SignatureTypeKey::Binder { depth: 0, index: 0 }]
    ));

    let enumeration = enum_id(&module, "ProjectedCallableChoice");
    for variant_index in 0..2 {
        let variant = hir::EnumVariantRef::checked(&module.enums, enumeration, variant_index)
            .expect("fixture variant exists");
        let variant_id = module.enum_member_identities[variant].id();
        let record = callables
            .get(CallableTemplateOrigin::VariantConstructor(variant_id))
            .expect("every public variant has a callable interface");
        assert_eq!(
            record.parameters().parameters().len(),
            variant_index as usize
        );
        assert!(matches!(
            record.result(),
            SignatureTypeKey::NominalApplication { arguments, .. }
                if arguments.as_slice() == [SignatureTypeKey::Binder { depth: 0, index: 0 }]
        ));
    }

    let interface = module
        .interfaces
        .iter()
        .find_map(|(id, declaration)| {
            (declaration.name == "ProjectedCallableContract").then_some(id)
        })
        .unwrap();
    let modalities = module.interfaces[interface]
        .methods
        .iter()
        .map(|&member| {
            let function = module.interface_methods[member].function;
            (
                module.functions[function].name.rsplit('.').next().unwrap(),
                callables
                    .get(callable_id(&module, function))
                    .unwrap()
                    .modality(),
            )
        })
        .collect::<std::collections::BTreeMap<_, _>>();
    assert_eq!(modalities["required"], hir::CallableModalityV1::Abstract);
    assert_eq!(
        modalities["provided"],
        hir::CallableModalityV1::InterfaceDefault
    );

    let property = module
        .properties
        .iter()
        .find_map(|(id, property)| (property.name == "ProjectedMutable").then_some(id))
        .unwrap();
    let getter = module.properties[property].capability.getter();
    let getter_id = module.property_accessor_identities[getter].id();
    let getter = callables
        .get(CallableTemplateOrigin::Accessor(getter_id))
        .unwrap();
    assert!(getter.parameters().is_empty());
    assert_eq!(getter.modality(), hir::CallableModalityV1::Final);
    let setter = module.properties[property].capability.setter().unwrap();
    let setter_id = module.property_accessor_identities[setter].id();
    let setter = callables
        .get(CallableTemplateOrigin::Accessor(setter_id))
        .unwrap();
    assert_eq!(setter.parameters().parameters().len(), 1);
    assert_eq!(setter.parameters().parameters()[0].name().as_str(), "value");
    assert_eq!(
        setter.result(),
        &SignatureTypeKey::Nominal(
            scoop_identity::CoreBuiltinNominal::Unit
                .identity_record()
                .id()
        )
    );
}

#[test]
fn producer_rejects_a_public_function_without_public_lookup() {
    let declarations = public_declarations(vec![fun_expr(
        "BrokenCallableProjection",
        Vec::new(),
        Vec::new(),
        Some(ty_named("Int")),
        int_lit(1),
    )]);
    let mut module = lower_core_with_additional_declarations(declarations).into_module();
    let function = function_id(&module, "BrokenCallableProjection");
    module.functions[function].access.declared = hir::DeclaredVisibility::Internal;

    assert!(matches!(
        hir::CanonicalCallableInterfacesV1::from_export_hir(&module),
        Err(hir::CallableInterfaceBuildError::Projection {
            subject: hir::CallableProjectionSubject::Function(_),
            error: hir::CallableProjectionError::Access(
                hir::CallableAccessProjectionError::NotDeclaredPublic
            ),
        })
    ));
}
