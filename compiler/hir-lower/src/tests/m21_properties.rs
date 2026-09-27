use super::*;

fn getter(expression: Expr) -> ast::AccessorSyntax {
    ast::AccessorSyntax {
        getter: Some(ast::GetterDecl {
            annotations: Vec::new(),
            body: ast::AccessorBodySyntax::Expr(Box::new(expression)),
            span: sp(),
        }),
        setter: None,
    }
}

fn property(
    name: &str,
    mutable: bool,
    modifier: ast::MethodModifier,
    is_override: bool,
    ty: TypeRef,
    body: ast::PropertyBodySyntax,
) -> ast::PropertyDecl {
    ast::PropertyDecl {
        annotations: Vec::new(),
        visibility: ast::VisibilitySyntax::Omitted,
        modifier,
        is_override,
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

fn class_with_properties(
    modifier: ast::ClassModifier,
    name: &str,
    base: Option<&str>,
    properties: Vec<ast::PropertyDecl>,
) -> Decl {
    let mut declaration = class_decl(
        modifier,
        name,
        Vec::new(),
        base.map(|base| (base, Vec::new())),
        Vec::new(),
        Vec::new(),
    );
    let Decl::Class(class) = &mut declaration else {
        unreachable!("class_decl constructs a class")
    };
    class.members = properties
        .into_iter()
        .map(ast::ClassMember::StoredProperty)
        .collect();
    declaration
}

#[test]
fn logical_properties_keep_accessors_and_storage_as_distinct_typed_entities() {
    let stored = property(
        "stored",
        true,
        ast::MethodModifier::Final,
        false,
        ty_named("Int"),
        ast::PropertyBodySyntax::Initializer {
            expression: Box::new(int_lit(1)),
            accessors: getter(var("field")),
        },
    );
    let computed = property(
        "computed",
        false,
        ast::MethodModifier::Final,
        false,
        ty_named("Int"),
        ast::PropertyBodySyntax::Computed(getter(int_lit(2))),
    );
    let optional = property(
        "optional",
        true,
        ast::MethodModifier::Final,
        false,
        ty_nullable(ty_named("Int")),
        ast::PropertyBodySyntax::OptionalOmitted,
    );
    let module = lower_user(file(vec![
        class_with_properties(
            ast::ClassModifier::Final,
            "Properties",
            None,
            vec![stored, computed, optional],
        ),
        fun("main", Vec::new()),
    ]))
    .expect("all core property representations lower");
    let (_, class) = module
        .classes
        .iter()
        .find(|(_, class)| class.name == "Properties")
        .expect("Properties class");
    assert_eq!(class.properties.len(), 3);
    assert_eq!(class.fields.len(), 2);

    let stored = &module.properties[class.properties[0]];
    let hir::PropertyRepresentation::Stored(storage) = &stored.representation else {
        panic!("stored property has physical storage")
    };
    let hir::PropertyBacking::ClassField { field, initializer } = storage.backing else {
        panic!("class property has a class backing field")
    };
    assert_eq!(module.class_fields[field].property, class.properties[0]);
    assert_eq!(initializer, hir::ClassPropertyInitializer::Expression);
    let getter = module.property_getters[stored.capability.getter()].implementation;
    assert!(matches!(
        getter,
        hir::PropertyAccessorImplementation::Body(_)
    ));
    let setter = stored.capability.setter().expect("mutable property setter");
    assert_eq!(
        module.property_setters[setter].implementation,
        hir::PropertyAccessorImplementation::Storage
    );

    let computed = &module.properties[class.properties[1]];
    assert!(matches!(
        computed.representation,
        hir::PropertyRepresentation::AccessorOnly
    ));
    assert!(matches!(
        module.property_getters[computed.capability.getter()].implementation,
        hir::PropertyAccessorImplementation::Body(_)
    ));

    let optional = &module.properties[class.properties[2]];
    let hir::PropertyRepresentation::Stored(optional) = &optional.representation else {
        panic!("Option shorthand is stored")
    };
    assert!(matches!(
        optional.backing,
        hir::PropertyBacking::ClassField {
            initializer: hir::ClassPropertyInitializer::SyntheticNone,
            ..
        }
    ));
}

#[test]
fn property_override_owns_one_virtual_accessor_family_and_visibility_witness() {
    let base = property(
        "number",
        false,
        ast::MethodModifier::Open,
        false,
        ty_named("Int"),
        ast::PropertyBodySyntax::Computed(getter(int_lit(1))),
    );
    let derived = property(
        "number",
        false,
        ast::MethodModifier::Open,
        true,
        ty_named("Int"),
        ast::PropertyBodySyntax::Computed(getter(int_lit(2))),
    );
    let module = lower_user(file(vec![
        class_with_properties(ast::ClassModifier::Open, "Base", None, vec![base]),
        class_with_properties(
            ast::ClassModifier::Final,
            "Derived",
            Some("Base"),
            vec![derived],
        ),
        fun("main", Vec::new()),
    ]))
    .expect("a legal property override lowers");
    let (_, base) = module
        .classes
        .iter()
        .find(|(_, class)| class.name == "Base")
        .expect("Base class");
    let (_, derived) = module
        .classes
        .iter()
        .find(|(_, class)| class.name == "Derived")
        .expect("Derived class");
    let base_property = base.properties[0];
    let derived_property = derived.properties[0];
    assert_eq!(
        module.properties[derived_property].overrides,
        [base_property]
    );

    let getter_function = |property: hir::PropertyId| {
        let getter = module.properties[property].capability.getter();
        match module.property_getters[getter].implementation {
            hir::PropertyAccessorImplementation::Body(function)
            | hir::PropertyAccessorImplementation::AbstractSlot(function) => function,
            hir::PropertyAccessorImplementation::Storage
            | hir::PropertyAccessorImplementation::Constant => {
                panic!("open property accessors are callable")
            }
        }
    };
    let base_method = module.functions[getter_function(base_property)]
        .method
        .expect("base getter method");
    let derived_method = module.functions[getter_function(derived_property)]
        .method
        .expect("derived getter method");
    let hir::MethodDispatch::Virtual(base_family) = base_method.dispatch else {
        panic!("base getter starts a virtual family")
    };
    assert_eq!(
        derived_method.dispatch,
        hir::MethodDispatch::FinalOverride(base_family)
    );
}

#[test]
fn extension_property_owns_a_distinct_receiver_template_and_accessor() {
    let mut extension = property(
        "answer",
        false,
        ast::MethodModifier::Final,
        false,
        ty_named("Int"),
        ast::PropertyBodySyntax::Computed(getter(int_lit(42))),
    );
    extension.receiver_ty = Some(ty_named("Int"));
    let module = lower_user(file(vec![
        ast::Decl::Global(extension),
        fun("main", Vec::new()),
    ]))
    .expect("a computed extension property lowers");
    let (property, declaration) = module
        .properties
        .iter()
        .find(|(_, property)| property.name == "answer")
        .expect("logical extension property");
    let hir::PropertyOwner::Extension(extension) = declaration.owner else {
        panic!("extension property has a typed extension owner")
    };
    let template = &module.extension_properties[extension];
    assert_eq!(template.property, property);
    assert_eq!(template.receiver_ty, int_type(&module));
    assert!(template.type_params.is_empty());
    let getter = match module.property_getters[declaration.capability.getter()].implementation {
        hir::PropertyAccessorImplementation::Body(function) => function,
        _ => panic!("computed extension getter has a body"),
    };
    assert!(module.functions[getter].method.is_none());
    assert_eq!(module.functions[getter].params[0].name, "this");
    assert_eq!(module.functions[getter].params[0].ty, int_type(&module));
}
