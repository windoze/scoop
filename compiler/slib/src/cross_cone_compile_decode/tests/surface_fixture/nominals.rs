use scoop_hir::{
    PropertyAccessorImplementationV1 as AccessorForm, PropertyAccessorSourceV1 as AccessorSource,
    PropertyAccessorsV1 as Accessors,
};

use super::*;

pub(in crate::cross_cone_compile_decode::tests) fn nominal_surface(
    cone: scoop_identity::ConeIdentity,
    matching_callable_parameter: bool,
    matching_property_owner: bool,
    include_public_accessors: bool,
) -> (
    CanonicalHirFoundation,
    Vec<u8>,
    PersistentTypeId,
    PersistentFunctionId,
    PersistentConstructorId,
    PersistentPropertyId,
    PersistentExtensionPropertyId,
    PersistentPropertyAccessorId,
) {
    let source =
        SourceIdentity::new(cone, NormalizedSourcePath::new("src/Value.scoop").unwrap()).unwrap();
    let context_key = SourceContextKey::File {
        source: source.clone(),
    };
    let context =
        CborIdentityRecord::<PersistentSourceContextId, _>::from_key(context_key.clone()).unwrap();
    let origin =
        DefinitionOrigin::new(source.clone(), SourceSpan::new(0, 5).unwrap(), &context_key)
            .unwrap();
    let top_level_site = SourceDeclarationSite::new(
        cone,
        PackagePath::root(),
        DefinitionOwnerChain::top_level(),
        DeclarationScope::ConeWide,
    )
    .unwrap();
    let declaration = SourceDeclarationKey::nominal(
        top_level_site.clone(),
        CanonicalIdentifier::new("Value").unwrap(),
        SourceNominalKind::Struct,
        0,
    );
    let nominal = CborIdentityRecord::<PersistentTypeId, _>::from_key(declaration).unwrap();
    let owned_site = SourceDeclarationSite::new(
        cone,
        PackagePath::root(),
        DefinitionOwnerChain::from_outer_to_inner(vec![DefinitionOwnerAtom::Type(nominal.id())]),
        DeclarationScope::ConeWide,
    )
    .unwrap();
    let constructor = CborIdentityRecord::<PersistentConstructorId, _>::from_key(
        SourceDeclarationKey::constructor(owned_site.clone(), Vec::new()),
    )
    .unwrap();
    let member =
        CborIdentityRecord::<PersistentFunctionId, _>::from_key(SourceDeclarationKey::function(
            owned_site,
            CanonicalIdentifier::new("size").unwrap(),
            0,
            None,
            vec![SignatureTypeKey::Nominal(nominal.id())],
        ))
        .unwrap();
    let extension = CborIdentityRecord::<PersistentGenericFunctionId, _>::from_key(
        SourceDeclarationKey::function(
            SourceDeclarationSite::new(
                cone,
                PackagePath::root(),
                DefinitionOwnerChain::top_level(),
                DeclarationScope::ConeWide,
            )
            .unwrap(),
            CanonicalIdentifier::new("mapValue").unwrap(),
            1,
            Some(SignatureTypeKey::Nominal(nominal.id())),
            vec![SignatureTypeKey::Binder { depth: 0, index: 0 }],
        ),
    )
    .unwrap();
    let property =
        CborIdentityRecord::<PersistentPropertyId, _>::from_key(SourceDeclarationKey::property(
            top_level_site.clone(),
            CanonicalIdentifier::new("current").unwrap(),
        ))
        .unwrap();
    let getter = CborIdentityRecord::<PersistentPropertyAccessorId, _>::from_key(
        PropertyAccessorKey::new(PropertyOwner::Property(property.id()), AccessorRole::Getter),
    )
    .unwrap();
    let extension_property = CborIdentityRecord::<PersistentExtensionPropertyId, _>::from_key(
        SourceDeclarationKey::extension_property(
            top_level_site,
            CanonicalIdentifier::new("mapped").unwrap(),
            1,
            SignatureTypeKey::Nominal(nominal.id()),
        ),
    )
    .unwrap();
    let extension_getter =
        CborIdentityRecord::<PersistentPropertyAccessorId, _>::from_key(PropertyAccessorKey::new(
            PropertyOwner::ExtensionProperty(extension_property.id()),
            AccessorRole::Getter,
        ))
        .unwrap();
    let extension_setter =
        CborIdentityRecord::<PersistentPropertyAccessorId, _>::from_key(PropertyAccessorKey::new(
            PropertyOwner::ExtensionProperty(extension_property.id()),
            AccessorRole::Setter,
        ))
        .unwrap();
    let field = CborIdentityRecord::<PersistentFieldId, _>::from_key(
        FieldIdentityKey::source_declared(
            nominal.key(),
            CanonicalIdentifier::new("payload").unwrap(),
        )
        .unwrap(),
    )
    .unwrap();

    let mut foundation = base_hir_foundation();
    foundation
        .set_sources(vec![
            scoop_hir::SourceRecord::from_utf8(source, "class Value", [0, 5]).unwrap(),
        ])
        .unwrap();
    foundation.set_source_contexts(vec![context]).unwrap();
    foundation
        .set_definition_origins(vec![
            DefinitionOriginRecord::new(
                DefinitionOriginSubject::Type(nominal.id()),
                origin.clone(),
            ),
            DefinitionOriginRecord::new(
                DefinitionOriginSubject::Constructor(constructor.id()),
                origin.clone(),
            ),
            DefinitionOriginRecord::new(
                DefinitionOriginSubject::Function(member.id()),
                origin.clone(),
            ),
            DefinitionOriginRecord::new(
                DefinitionOriginSubject::GenericFunction(extension.id()),
                origin.clone(),
            ),
            DefinitionOriginRecord::new(
                DefinitionOriginSubject::Property(property.id()),
                origin.clone(),
            ),
            DefinitionOriginRecord::new(
                DefinitionOriginSubject::PropertyAccessor(getter.id()),
                origin.clone(),
            ),
            DefinitionOriginRecord::new(
                DefinitionOriginSubject::ExtensionProperty(extension_property.id()),
                origin.clone(),
            ),
            DefinitionOriginRecord::new(
                DefinitionOriginSubject::PropertyAccessor(extension_getter.id()),
                origin.clone(),
            ),
            DefinitionOriginRecord::new(
                DefinitionOriginSubject::PropertyAccessor(extension_setter.id()),
                origin.clone(),
            ),
            DefinitionOriginRecord::new(DefinitionOriginSubject::Field(field.id()), origin),
        ])
        .unwrap();
    foundation
        .set_types(vec![
            CoreBuiltinNominal::Unit.identity_record(),
            CoreBuiltinNominal::Any.identity_record(),
            nominal.clone(),
        ])
        .unwrap();
    foundation
        .set_constructors(vec![constructor.clone()])
        .unwrap();
    foundation.set_functions(vec![member.clone()]).unwrap();
    foundation
        .set_generic_functions(vec![extension.clone()])
        .unwrap();
    foundation.set_properties(vec![property.clone()]).unwrap();
    foundation
        .set_extension_properties(vec![extension_property.clone()])
        .unwrap();
    foundation
        .set_property_accessors(vec![
            getter.clone(),
            extension_getter.clone(),
            extension_setter.clone(),
        ])
        .unwrap();
    foundation.set_fields(vec![field.clone()]).unwrap();

    let record = crate::nominal_interface_fixture::public_record(
        SourceNominalId::Concrete(nominal.id()),
        PublicNominalKindV1::Struct,
        CanonicalBinderListV1::try_new(Vec::new()).unwrap(),
        CanonicalSignatureTypesV1::try_new(Vec::new()).unwrap(),
        CanonicalPersistentIdsV1::try_new(vec![constructor.id()]).unwrap(),
        CanonicalPublicMemberRefsV1::try_new(vec![PublicMemberRefV1::Callable(
            CallableTemplateOrigin::Function(member.id()),
        )])
        .unwrap(),
        CanonicalPersistentIdsV1::try_new(Vec::new()).unwrap(),
        NominalSourceShapeV1::Struct(
            StructSourceShapeV1::try_new(
                vec![NominalSourceFieldV1::new(
                    field.id(),
                    SignatureTypeKey::Nominal(nominal.id()),
                )],
                scoop_hir::NominalCLayoutPolicyV1::Ordinary,
                false,
            )
            .unwrap(),
        ),
    )
    .unwrap();
    let callable_parameter = if matching_callable_parameter {
        SignatureTypeKey::Nominal(nominal.id())
    } else {
        SignatureTypeKey::Nominal(CoreBuiltinNominal::Unit.identity_record().id())
    };
    let callable = CallableInterfaceRecordV1::try_new(
        CallableTemplateOrigin::Function(member.id()),
        PublicDeclarationOwnerV1::Nominal(SourceNominalId::Concrete(nominal.id())),
        CanonicalBinderListV1::try_new(Vec::new()).unwrap(),
        None,
        CanonicalSourceParameterShapesV1::try_new(vec![SourceParameterShapeV1::new(
            CanonicalIdentifier::new("value").unwrap(),
            callable_parameter,
        )])
        .unwrap(),
        SignatureTypeKey::Nominal(nominal.id()),
        scoop_effects(),
        CallableModalityV1::Final,
        PublicLookupAccessV1::DirectOnly,
        scoop_hir::CanonicalPersistentIdsV1::empty(),
    )
    .unwrap();
    let extension_callable = CallableInterfaceRecordV1::try_new(
        CallableTemplateOrigin::GenericFunction(extension.id()),
        PublicDeclarationOwnerV1::Extension,
        CanonicalBinderListV1::try_new(vec![TypeParameterBinderV1::new(
            CanonicalIdentifier::new("T").unwrap(),
            TypeParameterBoundsV1::Unconstrained,
        )])
        .unwrap(),
        Some(SignatureTypeKey::Nominal(nominal.id())),
        CanonicalSourceParameterShapesV1::try_new(vec![SourceParameterShapeV1::new(
            CanonicalIdentifier::new("mapped").unwrap(),
            SignatureTypeKey::Binder { depth: 0, index: 0 },
        )])
        .unwrap(),
        SignatureTypeKey::Binder { depth: 0, index: 0 },
        scoop_effects(),
        CallableModalityV1::Final,
        PublicLookupAccessV1::DirectOnly,
        scoop_hir::CanonicalPersistentIdsV1::empty(),
    )
    .unwrap();
    let constructor_callable = CallableInterfaceRecordV1::try_new(
        CallableTemplateOrigin::Constructor(constructor.id()),
        PublicDeclarationOwnerV1::Nominal(SourceNominalId::Concrete(nominal.id())),
        CanonicalBinderListV1::try_new(Vec::new()).unwrap(),
        None,
        CanonicalSourceParameterShapesV1::try_new(Vec::new()).unwrap(),
        SignatureTypeKey::Nominal(nominal.id()),
        scoop_effects(),
        CallableModalityV1::Final,
        PublicLookupAccessV1::DirectOnly,
        scoop_hir::CanonicalPersistentIdsV1::empty(),
    )
    .unwrap();
    let property_owner = if matching_property_owner {
        PublicDeclarationOwnerV1::TopLevel
    } else {
        PublicDeclarationOwnerV1::Nominal(SourceNominalId::Concrete(nominal.id()))
    };
    let getter_callable = CallableInterfaceRecordV1::try_new(
        CallableTemplateOrigin::Accessor(getter.id()),
        property_owner,
        CanonicalBinderListV1::try_new(Vec::new()).unwrap(),
        None,
        CanonicalSourceParameterShapesV1::try_new(Vec::new()).unwrap(),
        SignatureTypeKey::Nominal(nominal.id()),
        scoop_effects(),
        CallableModalityV1::Final,
        PublicLookupAccessV1::DirectOnly,
        scoop_hir::CanonicalPersistentIdsV1::empty(),
    )
    .unwrap();
    let extension_getter_callable = CallableInterfaceRecordV1::try_new(
        CallableTemplateOrigin::Accessor(extension_getter.id()),
        PublicDeclarationOwnerV1::Extension,
        CanonicalBinderListV1::try_new(Vec::new()).unwrap(),
        Some(SignatureTypeKey::Nominal(nominal.id())),
        CanonicalSourceParameterShapesV1::try_new(Vec::new()).unwrap(),
        SignatureTypeKey::Binder { depth: 0, index: 0 },
        scoop_effects(),
        CallableModalityV1::Final,
        PublicLookupAccessV1::DirectOnly,
        scoop_hir::CanonicalPersistentIdsV1::empty(),
    )
    .unwrap();
    let property_record = PropertyInterfaceRecordV1::try_new(
        PropertyOwner::Property(property.id()),
        property_owner,
        CanonicalBinderListV1::try_new(Vec::new()).unwrap(),
        None,
        SignatureTypeKey::Nominal(nominal.id()),
        Accessors::read_only(AccessorSource::new(getter.id(), AccessorForm::Body)),
        PropertyRepresentationV1::RuntimeAccessor,
        PropertyPublicAccessV1::DirectOnly,
        scoop_hir::PropertySetterPublicAccessV1::Restricted,
    )
    .unwrap();
    let extension_property_record = PropertyInterfaceRecordV1::try_new(
        PropertyOwner::ExtensionProperty(extension_property.id()),
        PublicDeclarationOwnerV1::Extension,
        CanonicalBinderListV1::try_new(vec![TypeParameterBinderV1::new(
            CanonicalIdentifier::new("T").unwrap(),
            TypeParameterBoundsV1::Unconstrained,
        )])
        .unwrap(),
        Some(SignatureTypeKey::Nominal(nominal.id())),
        SignatureTypeKey::Binder { depth: 0, index: 0 },
        Accessors::try_read_write(
            AccessorSource::new(extension_getter.id(), AccessorForm::Body),
            AccessorSource::new(extension_setter.id(), AccessorForm::Body),
        )
        .unwrap(),
        PropertyRepresentationV1::RuntimeAccessor,
        PropertyPublicAccessV1::DirectOnly,
        scoop_hir::PropertySetterPublicAccessV1::Restricted,
    )
    .unwrap();
    let restricted_setter = scoop_hir::CallableDeclarationRecordV1::try_new(
        CallableTemplateOrigin::Accessor(extension_setter.id()),
        PublicDeclarationOwnerV1::Extension,
        CanonicalBinderListV1::try_new(Vec::new()).unwrap(),
        Some(SignatureTypeKey::Nominal(nominal.id())),
        CanonicalSourceParameterShapesV1::try_new(vec![SourceParameterShapeV1::new(
            CanonicalIdentifier::new("next").unwrap(),
            SignatureTypeKey::Binder { depth: 0, index: 0 },
        )])
        .unwrap(),
        SignatureTypeKey::Nominal(CoreBuiltinNominal::Unit.identity_record().id()),
        scoop_effects(),
        CallableModalityV1::Final,
        scoop_hir::DeclaredVisibilityV1::Private,
        CanonicalPersistentIdsV1::empty(),
    )
    .unwrap();
    let mut callables = vec![callable, constructor_callable, extension_callable];
    if include_public_accessors {
        callables.push(getter_callable);
        callables.push(extension_getter_callable);
    }
    (
        foundation,
        interface_with_support(
            vec![record],
            callables,
            vec![property_record, extension_property_record],
            vec![restricted_setter],
        ),
        nominal.id(),
        member.id(),
        constructor.id(),
        property.id(),
        extension_property.id(),
        getter.id(),
    )
}
