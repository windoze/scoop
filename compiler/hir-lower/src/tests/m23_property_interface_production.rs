use scoop_identity::{
    AccessorRole, NominalDeclarationOwner, PropertyOwner as PersistentPropertyOwner,
};

use super::*;

#[derive(Clone, Copy)]
enum SetterVisibility {
    None,
    Public,
    Private,
}

fn public_visibility() -> ast::VisibilitySyntax {
    ast::VisibilitySyntax::Explicit {
        visibility: ast::DeclaredVisibility::Public,
        span: sp(),
    }
}

fn computed_property(
    name: &str,
    receiver_ty: Option<TypeRef>,
    type_params: Vec<ast::TypeParamDecl>,
    setter: SetterVisibility,
) -> ast::PropertyDecl {
    let setter = match setter {
        SetterVisibility::None => None,
        SetterVisibility::Public | SetterVisibility::Private => Some(ast::SetterDecl {
            annotations: Vec::new(),
            visibility: match setter {
                SetterVisibility::Public => ast::SetterVisibilitySyntax::Inherited,
                SetterVisibility::Private => ast::SetterVisibilitySyntax::Explicit {
                    visibility: ast::DeclaredVisibility::Private,
                    span: sp(),
                },
                SetterVisibility::None => unreachable!(),
            },
            parameter: ast::SetterParameterSyntax::Default { span: sp() },
            body: ast::AccessorBodySyntax::Block(block(Vec::new())),
            span: sp(),
        }),
    };
    ast::PropertyDecl {
        annotations: Vec::new(),
        visibility: public_visibility(),
        modifier: ast::MethodModifier::Final,
        is_override: false,
        mutable: setter.is_some(),
        receiver_ty,
        type_params,
        where_clause: None,
        name: ident(name),
        ty: ty_named("Int"),
        body: ast::PropertyBodySyntax::Computed(ast::AccessorSyntax {
            getter: Some(ast::GetterDecl {
                annotations: Vec::new(),
                body: ast::AccessorBodySyntax::Expr(Box::new(int_lit(7))),
                span: sp(),
            }),
            setter,
        }),
        span: sp(),
    }
}

fn abstract_property(name: &str, ty: TypeRef) -> ast::PropertyDecl {
    ast::PropertyDecl {
        ty,
        body: ast::PropertyBodySyntax::Abstract,
        modifier: ast::MethodModifier::Abstract,
        ..computed_property(name, None, Vec::new(), SetterVisibility::None)
    }
}

fn object_const(name: &str) -> Decl {
    let property = ast::PropertyDecl {
        annotations: Vec::new(),
        visibility: public_visibility(),
        modifier: ast::MethodModifier::Final,
        is_override: false,
        mutable: false,
        receiver_ty: None,
        type_params: Vec::new(),
        where_clause: None,
        name: ident("constant"),
        ty: ty_named("Int"),
        body: ast::PropertyBodySyntax::Const(Box::new(int_lit(9))),
        span: sp(),
    };
    Decl::Object(ast::ObjectDecl {
        annotations: Vec::new(),
        visibility: public_visibility(),
        name: ident(name),
        supertypes: Vec::new(),
        members: vec![ast::ClassMember::StoredProperty(property)],
        span: sp(),
    })
}

fn public_interface_with_abstract_property(name: &str) -> Decl {
    let mut declaration = generic_interface_decl(name, vec!["T"], Vec::new());
    let Decl::Interface(interface) = &mut declaration else {
        unreachable!()
    };
    interface.visibility = public_visibility();
    interface
        .properties
        .push(abstract_property("abstractValue", ty_named("T")));
    declaration
}

fn lowered_property(module: &hir::Module, name: &str) -> hir::PropertyId {
    module
        .properties
        .iter()
        .find_map(|(id, property)| (property.name == name).then_some(id))
        .unwrap_or_else(|| panic!("missing lowered property `{name}`"))
}

fn persistent_property(module: &hir::Module, property: hir::PropertyId) -> PersistentPropertyOwner {
    match &module.property_identities[property] {
        hir::HirPropertyIdentity::Ordinary(record) => {
            PersistentPropertyOwner::Property(record.id())
        }
        hir::HirPropertyIdentity::Extension(record) => {
            PersistentPropertyOwner::ExtensionProperty(record.id())
        }
    }
}

#[test]
fn producer_projects_property_owners_binders_capabilities_and_representations() {
    let module = lower_core_with_additional_declarations(vec![
        Decl::Global(computed_property(
            "PublicMutableProperty",
            None,
            Vec::new(),
            SetterVisibility::Public,
        )),
        Decl::Global(computed_property(
            "RestrictedSetterProperty",
            None,
            Vec::new(),
            SetterVisibility::Private,
        )),
        Decl::Global(computed_property(
            "GenericExtensionProperty",
            Some(ty_named("T")),
            vec![type_param("T")],
            SetterVisibility::None,
        )),
        public_interface_with_abstract_property("ProjectedPropertyContract"),
        object_const("ProjectedPropertyObject"),
    ]);
    let interfaces = hir::CanonicalPropertyInterfacesV1::from_export_hir(&module)
        .expect("the public property surface must project canonically");
    assert_eq!(
        interfaces.records().len(),
        module.public_surface.properties.len()
    );

    let mutable = lowered_property(&module, "PublicMutableProperty");
    let mutable_record = interfaces
        .get(persistent_property(&module, mutable))
        .unwrap();
    assert_eq!(
        mutable_record.owner(),
        hir::PublicDeclarationOwnerV1::TopLevel
    );
    assert_eq!(
        mutable_record.access(),
        hir::PropertyPublicAccessV1::DirectOnly
    );
    assert_eq!(
        mutable_record.representation(),
        hir::PropertyRepresentationV1::RuntimeAccessor
    );
    assert_eq!(
        mutable_record.capability().setter_access(),
        Some(hir::PropertySetterPublicAccessV1::Public)
    );

    let restricted = lowered_property(&module, "RestrictedSetterProperty");
    let restricted_record = interfaces
        .get(persistent_property(&module, restricted))
        .unwrap();
    assert_eq!(
        restricted_record.capability().setter_access(),
        Some(hir::PropertySetterPublicAccessV1::Restricted)
    );

    let extension = lowered_property(&module, "GenericExtensionProperty");
    let extension_record = interfaces
        .get(persistent_property(&module, extension))
        .unwrap();
    assert_eq!(
        extension_record.owner(),
        hir::PublicDeclarationOwnerV1::Extension
    );
    assert_eq!(extension_record.type_parameters().binders().len(), 1);
    assert_eq!(
        extension_record.type_parameters().binders()[0]
            .name()
            .as_str(),
        "T"
    );
    assert_eq!(
        extension_record.receiver(),
        Some(&scoop_identity::SignatureTypeKey::Binder { depth: 0, index: 0 })
    );

    let abstract_property = lowered_property(&module, "abstractValue");
    let abstract_record = interfaces
        .get(persistent_property(&module, abstract_property))
        .unwrap();
    assert_eq!(
        abstract_record.access(),
        hir::PropertyPublicAccessV1::PublicSlot
    );
    assert_eq!(
        abstract_record.representation(),
        hir::PropertyRepresentationV1::AbstractSlot
    );
    assert!(abstract_record.type_parameters().is_empty());
    assert_eq!(
        abstract_record.value_type(),
        &scoop_identity::SignatureTypeKey::Binder { depth: 0, index: 0 }
    );
    let hir::PropertyOwner::Interface(interface) = module.properties[abstract_property].owner
    else {
        panic!("abstract property must retain its interface owner")
    };
    let interface_identity = module.nominal_identities[interface]
        .source()
        .and_then(hir::HirSourceNominalIdentity::generic_id)
        .unwrap();
    assert_eq!(
        abstract_record.owner(),
        hir::PublicDeclarationOwnerV1::Nominal(NominalDeclarationOwner::GenericTemplate(
            interface_identity
        ))
    );

    let object_property = lowered_property(&module, "constant");
    let object_record = interfaces
        .get(persistent_property(&module, object_property))
        .unwrap();
    let hir::PropertyOwner::Object(object) = module.properties[object_property].owner else {
        panic!("object const must retain its object owner")
    };
    let object_identity = module.nominal_identities[object]
        .source()
        .and_then(hir::HirSourceNominalIdentity::concrete_id)
        .unwrap();
    assert_eq!(
        object_record.owner(),
        hir::PublicDeclarationOwnerV1::Nominal(NominalDeclarationOwner::Concrete(object_identity))
    );
    assert_eq!(
        object_record.representation(),
        hir::PropertyRepresentationV1::Const
    );
}

#[test]
fn producer_rejects_a_public_property_with_a_non_public_getter() {
    let mut module =
        lower_core_with_additional_declarations(vec![Decl::Global(computed_property(
            "BrokenGetterProperty",
            None,
            Vec::new(),
            SetterVisibility::None,
        ))])
        .into_module();
    let property = lowered_property(&module, "BrokenGetterProperty");
    let persistent = persistent_property(&module, property);
    let getter = module.properties[property].capability.getter();
    module.property_getters[getter].access.declared = hir::DeclaredVisibility::Internal;

    assert_eq!(
        hir::CanonicalPropertyInterfacesV1::from_export_hir(&module),
        Err(hir::PropertyInterfaceBuildError::Accessor {
            property: persistent,
            role: AccessorRole::Getter,
            detail: hir::ExportPropertyAccessorBuildError::DeclaredVisibilityMismatch {
                expected: hir::DeclaredVisibility::Public,
                actual: hir::DeclaredVisibility::Internal,
            },
        })
    );
}
