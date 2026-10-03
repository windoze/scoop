use scoop_identity::{
    BindableEntity, CallableTemplateOrigin, NominalDeclarationOwner,
    PropertyOwner as PersistentPropertyOwner, SignatureTypeKey,
};

use super::*;

fn public_declarations(declarations: Vec<Decl>) -> Vec<Decl> {
    let mut source = file(declarations);
    make_core_public(&mut source);
    source.declarations
}

fn projected_declarations() -> Vec<Decl> {
    let carrier = generic_interface_decl("ProjectedCarrier", vec!["T"], Vec::new());
    let base = class_decl(
        ast::ClassModifier::Open,
        "ProjectedBase",
        Vec::new(),
        None,
        Vec::new(),
        Vec::new(),
    );

    let mut class = class_decl(
        ast::ClassModifier::Final,
        "ProjectedClass",
        vec![(false, "value", ty_named("Int"))],
        Some(("ProjectedBase", Vec::new())),
        Vec::new(),
        vec![method_expr(
            "projectedClassMethod",
            Vec::new(),
            Some(ty_named("Int")),
            int_lit(1),
        )],
    );
    let Decl::Class(class_declaration) = &mut class else {
        unreachable!()
    };
    let ast::ClassConstructorDecl::Declared(constructor) = &mut class_declaration.constructor
    else {
        unreachable!()
    };
    constructor.parameters[0].syntax = ast::ParameterSyntax::Default {
        expression: int_lit(3),
        equals_span: sp(),
    };
    class_declaration.supertypes.push(bare_supertype(ty_generic(
        "ProjectedCarrier",
        vec![ty_named("Int")],
    )));

    let mut strukt =
        generic_struct_decl("ProjectedStruct", vec!["T"], vec![("value", ty_named("T"))]);
    let Decl::Struct(struct_declaration) = &mut strukt else {
        unreachable!()
    };
    struct_declaration
        .supertypes
        .push(bare_supertype(ty_generic(
            "ProjectedCarrier",
            vec![ty_named("T")],
        )));

    let mut enumeration = enum_decl(
        "ProjectedEnum",
        vec!["T"],
        vec![
            variant_unit("Empty"),
            variant_positional("Positional", vec![ty_named("T")]),
            variant_named("Named", vec![("value", ty_named("T"))]),
            variant_constructor("Constructed", vec![("value", ty_named("T"), None)]),
        ],
    );
    let Decl::Enum(enum_declaration) = &mut enumeration else {
        unreachable!()
    };
    enum_declaration
        .interfaces
        .push(ty_generic("ProjectedCarrier", vec![ty_named("T")]));

    let object = Decl::Object(ast::ObjectDecl {
        annotations: Vec::new(),
        visibility: ast::VisibilitySyntax::Omitted,
        name: ident("ProjectedObject"),
        supertypes: vec![bare_supertype(ty_generic(
            "ProjectedCarrier",
            vec![ty_named("Int")],
        ))],
        members: vec![ast::ClassMember::Function(method_expr(
            "projectedObjectMethod",
            Vec::new(),
            Some(ty_named("Int")),
            int_lit(2),
        ))],
        span: sp(),
    });

    let Decl::Struct(nested_struct) = struct_decl("NestedValue", Vec::new()) else {
        unreachable!()
    };
    let nested_object = ast::ObjectDecl {
        annotations: Vec::new(),
        visibility: ast::VisibilitySyntax::Omitted,
        name: ident("NestedSingleton"),
        supertypes: Vec::new(),
        members: Vec::new(),
        span: sp(),
    };
    let mut outer = class_decl(
        ast::ClassModifier::Final,
        "ProjectedOuter",
        Vec::new(),
        None,
        Vec::new(),
        Vec::new(),
    );
    let Decl::Class(outer_decl) = &mut outer else {
        unreachable!()
    };
    outer_decl.members.extend([
        ast::ClassMember::Nested(Box::new(ast::NestedNominalDecl::Struct(Box::new(
            nested_struct,
        )))),
        ast::ClassMember::Nested(Box::new(ast::NestedNominalDecl::Object(Box::new(
            nested_object,
        )))),
    ]);

    public_declarations(vec![
        carrier,
        base,
        class,
        strukt,
        enumeration,
        object,
        outer,
    ])
}

fn source_nominal(identity: &hir::HirNominalIdentity) -> NominalDeclarationOwner {
    match identity.source().expect("test declaration is source-owned") {
        hir::HirSourceNominalIdentity::Concrete(record) => {
            NominalDeclarationOwner::Concrete(record.id())
        }
        hir::HirSourceNominalIdentity::Generic(record) => {
            NominalDeclarationOwner::GenericTemplate(record.id())
        }
    }
}

fn class_id(module: &hir::Module, name: &str) -> hir::ClassId {
    module
        .classes
        .iter()
        .find_map(|(id, declaration)| (declaration.name == name).then_some(id))
        .unwrap_or_else(|| panic!("missing class `{name}`"))
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

fn interface_id(module: &hir::Module, name: &str) -> hir::InterfaceId {
    module
        .interfaces
        .iter()
        .find_map(|(id, declaration)| (declaration.name == name).then_some(id))
        .unwrap_or_else(|| panic!("missing interface `{name}`"))
}

fn object_id(module: &hir::Module, name: &str) -> hir::ObjectId {
    module
        .objects
        .iter()
        .find_map(|(id, declaration)| (declaration.name == name).then_some(id))
        .unwrap_or_else(|| panic!("missing object `{name}`"))
}

#[test]
fn producer_projects_complete_nominal_interfaces() {
    let module = lower_core_with_additional_declarations(projected_declarations());
    let interfaces = hir::CanonicalNominalInterfacesV1::from_export_hir(&module)
        .expect("the complete public nominal surface must project canonically");
    assert_eq!(
        interfaces.records().len(),
        module.public_surface.classes.len()
            + module.public_surface.interfaces.len()
            + module.public_surface.structs.len()
            + module.public_surface.enums.len()
            + module.public_surface.objects.len()
    );

    let carrier = interface_id(&module, "ProjectedCarrier");
    let carrier_declaration = source_nominal(&module.nominal_identities[carrier]);
    let carrier_record = interfaces.get(carrier_declaration).unwrap();
    assert_eq!(carrier_record.kind(), hir::PublicNominalKindV1::Interface);
    assert_eq!(carrier_record.type_parameters().binders().len(), 1);
    assert_eq!(
        carrier_record.type_parameters().binders()[0]
            .name()
            .as_str(),
        "T"
    );
    assert!(matches!(
        carrier_record.source_shape(),
        hir::NominalSourceShapeV1::Interface
    ));

    let strukt = struct_id(&module, "ProjectedStruct");
    let struct_record = interfaces
        .get(source_nominal(&module.nominal_identities[strukt]))
        .unwrap();
    assert_eq!(struct_record.kind(), hir::PublicNominalKindV1::Struct);
    assert_eq!(struct_record.constructors().values().len(), 1);
    assert!(struct_record.exact_supertypes().values().iter().any(|ty| {
        matches!(
            ty,
            SignatureTypeKey::NominalApplication { arguments, .. }
                if arguments.as_slice() == [SignatureTypeKey::Binder { depth: 0, index: 0 }]
        )
    }));
    let hir::NominalSourceShapeV1::Struct(shape) = struct_record.source_shape() else {
        panic!("ProjectedStruct must retain its struct source shape")
    };
    assert_eq!(shape.fields().len(), 1);
    assert_eq!(
        shape.fields()[0].value_type(),
        &SignatureTypeKey::Binder { depth: 0, index: 0 }
    );

    let enumeration = enum_id(&module, "ProjectedEnum");
    let enum_record = interfaces
        .get(source_nominal(&module.nominal_identities[enumeration]))
        .unwrap();
    assert!(enum_record.constructors().is_empty());
    let hir::NominalSourceShapeV1::Enum(shape) = enum_record.source_shape() else {
        panic!("ProjectedEnum must retain its enum source shape")
    };
    assert_eq!(
        shape
            .variants()
            .iter()
            .map(hir::EnumSourceVariantV1::style)
            .collect::<Vec<_>>(),
        [
            hir::EnumSourceVariantStyleV1::Unit,
            hir::EnumSourceVariantStyleV1::Positional,
            hir::EnumSourceVariantStyleV1::Named,
            hir::EnumSourceVariantStyleV1::Constructor,
        ]
    );
    assert_eq!(
        shape
            .variants()
            .iter()
            .map(|variant| variant.fields().len())
            .collect::<Vec<_>>(),
        [0, 1, 1, 1]
    );
    assert!(shape.variants()[1..].iter().all(|variant| {
        variant.fields()[0].value_type() == &SignatureTypeKey::Binder { depth: 0, index: 0 }
    }));

    let class = class_id(&module, "ProjectedClass");
    let class_record = interfaces
        .get(source_nominal(&module.nominal_identities[class]))
        .unwrap();
    assert_eq!(class_record.kind(), hir::PublicNominalKindV1::Class);
    assert_eq!(class_record.constructors().values().len(), 1);
    assert_eq!(class_record.exact_supertypes().values().len(), 2);
    assert_eq!(class_record.members().members().len(), 2);
    assert!(matches!(
        class_record.source_shape(),
        hir::NominalSourceShapeV1::Class(_)
    ));

    let object = object_id(&module, "ProjectedObject");
    let object_record = interfaces
        .get(source_nominal(&module.nominal_identities[object]))
        .unwrap();
    assert!(object_record.constructors().is_empty());
    assert_eq!(object_record.members().members().len(), 1);
    let hir::NominalSourceShapeV1::Object(shape) = object_record.source_shape() else {
        panic!("ProjectedObject must retain its singleton source shape")
    };
    assert_eq!(
        shape.value(),
        module.object_value_identities[module.objects[object].singleton_value].id()
    );

    let outer = class_id(&module, "ProjectedOuter");
    let outer_record = interfaces
        .get(source_nominal(&module.nominal_identities[outer]))
        .unwrap();
    let nested_struct = struct_id(&module, "NestedValue");
    let nested_object = object_id(&module, "NestedSingleton");
    let nested_targets = [
        match source_nominal(&module.nominal_identities[nested_struct]) {
            NominalDeclarationOwner::Concrete(id) => BindableEntity::Type(id),
            NominalDeclarationOwner::GenericTemplate(id) => BindableEntity::GenericType(id),
        },
        match source_nominal(&module.nominal_identities[nested_object]) {
            NominalDeclarationOwner::Concrete(id) => BindableEntity::Type(id),
            NominalDeclarationOwner::GenericTemplate(id) => BindableEntity::GenericType(id),
        },
        BindableEntity::ObjectValue(
            module.object_value_identities[module.objects[nested_object].singleton_value].id(),
        ),
    ];
    for target in nested_targets {
        let binding = module
            .export_binding_identities
            .iter()
            .find_map(|record| (record.key().target() == target).then_some(record.id()))
            .expect("every public nested target has a binding identity");
        assert!(outer_record.nested_bindings().values().contains(&binding));
    }
    assert_eq!(outer_record.nested_bindings().values().len(), 3);

    let class_method = module.classes[class]
        .methods
        .iter()
        .find(|&&id| module.functions[id].name.ends_with(".projectedClassMethod"))
        .copied()
        .unwrap_or_else(|| {
            panic!(
                "missing class method; declared methods are {:?}",
                module.classes[class]
                    .methods
                    .iter()
                    .map(|&id| module.functions[id].name.as_str())
                    .collect::<Vec<_>>()
            )
        });
    let hir::HirFunctionIdentity::Source(hir::HirSourceFunctionIdentity::Plain(identity)) =
        &module.function_identities[class_method]
    else {
        panic!("ProjectedClass method must have a plain source identity")
    };
    assert!(
        class_record
            .members()
            .members()
            .contains(&hir::PublicMemberRefV1::Callable(
                CallableTemplateOrigin::Function(identity.id())
            ))
    );
    let property = module.classes[class].properties[0];
    let hir::HirPropertyIdentity::Ordinary(identity) = &module.property_identities[property] else {
        panic!("ProjectedClass constructor property must be ordinary")
    };
    assert!(
        class_record
            .members()
            .members()
            .contains(&hir::PublicMemberRefV1::Property(
                PersistentPropertyOwner::Property(identity.id())
            ))
    );
}

#[test]
fn producer_rejects_a_public_nominal_with_non_public_access() {
    let declarations = public_declarations(vec![struct_decl(
        "BrokenNominalProjection",
        vec![("value", ty_named("Int"))],
    )]);
    let mut module = lower_core_with_additional_declarations(declarations).into_module();
    let strukt = struct_id(&module, "BrokenNominalProjection");
    let declaration = source_nominal(&module.nominal_identities[strukt]);
    module.structs[strukt].access.declared = hir::DeclaredVisibility::Internal;

    assert_eq!(
        hir::CanonicalNominalInterfacesV1::from_export_hir(&module),
        Err(hir::NominalInterfaceBuildError::InvalidPublicAccess(
            declaration
        ))
    );
}
