use scoop_hir::{
    CanonicalCallableInterfacesV1, CanonicalCallableSourceInterfacesV1,
    CanonicalExportConstValuesV1, CanonicalExportDefaultTemplatesV1,
    CanonicalExportDefinitionSourcesV1, CanonicalExternalHirReferencesV1,
    CanonicalNominalInterfacesV1, CanonicalPropertyInterfacesV1, CanonicalPublicExportBindingsV1,
    CanonicalTypeAliasInterfacesV1, CrossConeHirInterfaceSectionV1,
    ExternalHirReferenceSemanticAuthority, ExternalHirTargetV1,
};
use scoop_identity::{
    AccessorRole, BindingTarget, CallableTemplateOrigin, CanonicalIdentifier, CborIdentityRecord,
    ConeCoordinate, ConeIdentity, DeclarationScope, DefinitionOwnerAtom, DefinitionOwnerChain,
    EnumVariantFieldKey, EnumVariantFieldSelector, EnumVariantIdentityKey, FieldIdentityKey,
    GeneratedCallableKey, LexicalCallableParent, LexicalCallableRole, NominalDeclarationOwner,
    PackagePath, PendingIdentityValidation, PersistentConstructorId, PersistentEnumVariantFieldId,
    PersistentEnumVariantId, PersistentExtensionPropertyId, PersistentFieldId,
    PersistentFunctionId, PersistentGeneratedCallableId, PersistentGenericFunctionId,
    PersistentGenericTypeId, PersistentObjectValueId, PersistentPropertyAccessorId,
    PersistentPropertyId, PersistentTypeAliasId, PersistentTypeId, PropertyAccessorKey,
    PropertyOwner, SignatureTypeKey, SourceDeclarationKey, SourceDeclarationSite,
    SourceNominalKind, StructuralDefinitionPath, StructuralDefinitionSiteRole,
    StructuralPathSegment,
};

use super::*;

mod object_fields;

#[test]
fn resolves_every_source_backed_external_target_to_its_canonical_public_root() {
    let fixture = ExternalTargetFixture::new();
    let mut authority = CanonicalCrossConeRouteAuthority::try_new(
        fixture.current,
        &fixture.identities,
        &fixture.interface,
        &[],
        &[],
        &scoop_wire::WirePath::root(),
    )
    .unwrap();

    for (target, expected_root) in fixture.expected {
        assert_eq!(
            authority.external_hir_target_origin(target).unwrap(),
            fixture.provider,
            "wrong origin for {target:?}"
        );
        assert_eq!(
            authority.external_hir_target_binding_root(target).unwrap(),
            expected_root,
            "wrong binding root for {target:?}"
        );
    }
}

#[test]
fn rejects_a_generated_callable_without_a_lexical_source_root() {
    let fixture = ExternalTargetFixture::new();
    let target = ExternalHirTargetV1::GeneratedCallable(fixture.nonlexical_generated);
    let mut authority = CanonicalCrossConeRouteAuthority::try_new(
        fixture.current,
        &fixture.identities,
        &fixture.interface,
        &[],
        &[],
        &scoop_wire::WirePath::root(),
    )
    .unwrap();

    assert_eq!(
        authority.external_hir_target_binding_root(target),
        Err(CrossConeHirReferenceAuthorityError::NoPublicBindingRoot { target })
    );
}

struct ExternalTargetFixture {
    current: ConeIdentity,
    provider: ConeIdentity,
    identities: scoop_identity::ValidatedIdentityGraph,
    interface: CrossConeHirInterfaceSectionV1,
    expected: Vec<(ExternalHirTargetV1, BindingTarget)>,
    nonlexical_generated: PersistentGeneratedCallableId,
}

impl ExternalTargetFixture {
    fn new() -> Self {
        let current = cone("consumer");
        let provider = cone("provider");
        let struct_type =
            source_nominal::<PersistentTypeId>(provider, "Record", SourceNominalKind::Struct, 0);
        let generic_type =
            source_nominal::<PersistentGenericTypeId>(provider, "Box", SourceNominalKind::Class, 1);
        let enum_type =
            source_nominal::<PersistentTypeId>(provider, "Choice", SourceNominalKind::Enum, 0);
        let object_type =
            source_nominal::<PersistentTypeId>(provider, "Singleton", SourceNominalKind::Object, 0);

        let function = source_function::<PersistentFunctionId>(provider, "work", 0, None);
        let extension_function = source_function::<PersistentGenericFunctionId>(
            provider,
            "inspect",
            1,
            Some(SignatureTypeKey::Nominal(struct_type.id())),
        );
        let constructor = CborIdentityRecord::<PersistentConstructorId, _>::from_key(
            SourceDeclarationKey::constructor(
                source_site(
                    provider,
                    DefinitionOwnerChain::from_outer_to_inner(vec![DefinitionOwnerAtom::Type(
                        struct_type.id(),
                    )]),
                ),
                Vec::new(),
            ),
        )
        .unwrap();
        let property = CborIdentityRecord::<PersistentPropertyId, _>::from_key(
            SourceDeclarationKey::property(
                source_site(provider, DefinitionOwnerChain::top_level()),
                identifier("value"),
            ),
        )
        .unwrap();
        let extension_property = CborIdentityRecord::<PersistentExtensionPropertyId, _>::from_key(
            SourceDeclarationKey::extension_property(
                source_site(provider, DefinitionOwnerChain::top_level()),
                identifier("size"),
                0,
                SignatureTypeKey::Nominal(struct_type.id()),
            ),
        )
        .unwrap();
        let accessor = CborIdentityRecord::<PersistentPropertyAccessorId, _>::from_key(
            PropertyAccessorKey::new(PropertyOwner::Property(property.id()), AccessorRole::Getter),
        )
        .unwrap();
        let object_value =
            CborIdentityRecord::<PersistentObjectValueId, _>::from_key(object_type.key().clone())
                .unwrap();
        let alias = CborIdentityRecord::<PersistentTypeAliasId, _>::from_key(
            SourceDeclarationKey::type_alias(
                source_site(provider, DefinitionOwnerChain::top_level()),
                identifier("Alias"),
            ),
        )
        .unwrap();
        let field = CborIdentityRecord::<PersistentFieldId, _>::from_key(
            FieldIdentityKey::source_declared(struct_type.key(), identifier("field")).unwrap(),
        )
        .unwrap();
        let variant = CborIdentityRecord::<PersistentEnumVariantId, _>::from_key(
            EnumVariantIdentityKey::source(enum_type.key(), identifier("Some")).unwrap(),
        )
        .unwrap();
        let variant_field = CborIdentityRecord::<PersistentEnumVariantFieldId, _>::from_key(
            EnumVariantFieldKey::new(
                variant.id(),
                EnumVariantFieldSelector::Named(identifier("payload")),
            ),
        )
        .unwrap();
        let lexical_parent = CborIdentityRecord::<PersistentGeneratedCallableId, _>::from_key(
            GeneratedCallableKey::Lexical {
                parent: LexicalCallableParent::function(function.id()),
                role: LexicalCallableRole::LambdaBody,
                path: definition_path(0),
            },
        )
        .unwrap();
        let lexical_child = CborIdentityRecord::<PersistentGeneratedCallableId, _>::from_key(
            GeneratedCallableKey::CallableReferenceInvoke {
                parent: LexicalCallableParent::from_generated_key(lexical_parent.key()).unwrap(),
                path: definition_path(1),
            },
        )
        .unwrap();
        let constructor_adapter = CborIdentityRecord::<PersistentGeneratedCallableId, _>::from_key(
            GeneratedCallableKey::ZeroArgumentConstructorAdapter {
                constructor: constructor.id(),
            },
        )
        .unwrap();
        let exact = CborIdentityRecord::<scoop_identity::PersistentExactTypeId, _>::from_key(
            scoop_identity::ExactTypeKey::Nominal(struct_type.id()),
        )
        .unwrap();
        let nonlexical_generated =
            CborIdentityRecord::<PersistentGeneratedCallableId, _>::from_key(
                GeneratedCallableKey::DerivedEquality {
                    exact_owner: exact.id(),
                },
            )
            .unwrap();

        let expected = vec![
            (
                ExternalHirTargetV1::GeneratedCallable(constructor_adapter.id()),
                BindingTarget::type_name(struct_type.key()).unwrap(),
            ),
            (
                ExternalHirTargetV1::Nominal(NominalDeclarationOwner::Concrete(struct_type.id())),
                BindingTarget::type_name(struct_type.key()).unwrap(),
            ),
            (
                ExternalHirTargetV1::Nominal(NominalDeclarationOwner::GenericTemplate(
                    generic_type.id(),
                )),
                BindingTarget::type_name(generic_type.key()).unwrap(),
            ),
            (
                ExternalHirTargetV1::Callable(CallableTemplateOrigin::Function(function.id())),
                BindingTarget::function(function.key()).unwrap(),
            ),
            (
                ExternalHirTargetV1::Callable(CallableTemplateOrigin::GenericFunction(
                    extension_function.id(),
                )),
                BindingTarget::extension_function(extension_function.key()).unwrap(),
            ),
            (
                ExternalHirTargetV1::Callable(CallableTemplateOrigin::Constructor(
                    constructor.id(),
                )),
                BindingTarget::type_name(struct_type.key()).unwrap(),
            ),
            (
                ExternalHirTargetV1::Callable(CallableTemplateOrigin::Accessor(accessor.id())),
                BindingTarget::property(property.key()).unwrap(),
            ),
            (
                ExternalHirTargetV1::Callable(CallableTemplateOrigin::VariantConstructor(
                    variant.id(),
                )),
                BindingTarget::enum_variant(variant.id()),
            ),
            (
                ExternalHirTargetV1::Property(PropertyOwner::Property(property.id())),
                BindingTarget::property(property.key()).unwrap(),
            ),
            (
                ExternalHirTargetV1::Property(PropertyOwner::ExtensionProperty(
                    extension_property.id(),
                )),
                BindingTarget::extension_property(extension_property.key()).unwrap(),
            ),
            (
                ExternalHirTargetV1::ObjectValue(object_value.id()),
                BindingTarget::object_value(object_type.key()).unwrap(),
            ),
            (
                ExternalHirTargetV1::TypeAlias(alias.id()),
                BindingTarget::type_alias(alias.key()).unwrap(),
            ),
            (
                ExternalHirTargetV1::Field(field.id()),
                BindingTarget::type_name(struct_type.key()).unwrap(),
            ),
            (
                ExternalHirTargetV1::EnumVariantField(variant_field.id()),
                BindingTarget::enum_variant(variant.id()),
            ),
            (
                ExternalHirTargetV1::GeneratedCallable(lexical_child.id()),
                BindingTarget::function(function.key()).unwrap(),
            ),
        ];

        let nonlexical_id = nonlexical_generated.id();
        let mut pending = PendingIdentityValidation::new();
        register(&mut pending, struct_type);
        register(&mut pending, generic_type);
        register(&mut pending, enum_type);
        register(&mut pending, object_type);
        register(&mut pending, function);
        register(&mut pending, extension_function);
        register(&mut pending, constructor);
        register(&mut pending, property);
        register(&mut pending, extension_property);
        register(&mut pending, accessor);
        register(&mut pending, object_value);
        register(&mut pending, alias);
        register(&mut pending, field);
        register(&mut pending, variant);
        register(&mut pending, variant_field);
        register(&mut pending, lexical_parent);
        register(&mut pending, lexical_child);
        register(&mut pending, exact);
        register(&mut pending, constructor_adapter);
        register(&mut pending, nonlexical_generated);

        Self {
            current,
            provider,
            identities: pending.finish().unwrap(),
            interface: empty_interface(),
            expected,
            nonlexical_generated: nonlexical_id,
        }
    }
}

fn register<I, K>(pending: &mut PendingIdentityValidation, record: CborIdentityRecord<I, K>)
where
    I: scoop_identity::PersistentId + 'static,
    K: scoop_identity::CborIdentityKey<I> + Eq + Send + Sync + 'static,
{
    pending
        .register_external_canonical_authority(record)
        .unwrap();
}

fn source_nominal<I>(
    cone: ConeIdentity,
    name: &str,
    kind: SourceNominalKind,
    arity: u32,
) -> CborIdentityRecord<I, SourceDeclarationKey>
where
    I: scoop_identity::PersistentId + 'static,
    SourceDeclarationKey: scoop_identity::CborIdentityKey<I>,
    <SourceDeclarationKey as scoop_identity::CborIdentityKey<I>>::Error: std::fmt::Debug,
{
    CborIdentityRecord::from_key(SourceDeclarationKey::nominal(
        source_site(cone, DefinitionOwnerChain::top_level()),
        identifier(name),
        kind,
        arity,
    ))
    .unwrap()
}

fn source_function<I>(
    cone: ConeIdentity,
    name: &str,
    arity: u32,
    receiver: Option<SignatureTypeKey>,
) -> CborIdentityRecord<I, SourceDeclarationKey>
where
    I: scoop_identity::PersistentId + 'static,
    SourceDeclarationKey: scoop_identity::CborIdentityKey<I>,
    <SourceDeclarationKey as scoop_identity::CborIdentityKey<I>>::Error: std::fmt::Debug,
{
    CborIdentityRecord::from_key(SourceDeclarationKey::function(
        source_site(cone, DefinitionOwnerChain::top_level()),
        identifier(name),
        arity,
        receiver,
        Vec::new(),
    ))
    .unwrap()
}

fn source_site(cone: ConeIdentity, owners: DefinitionOwnerChain) -> SourceDeclarationSite {
    SourceDeclarationSite::new(
        cone,
        PackagePath::root(),
        owners,
        DeclarationScope::ConeWide,
    )
    .unwrap()
}

fn definition_path(ordinal: u32) -> StructuralDefinitionPath {
    StructuralDefinitionPath::from_first(
        StructuralPathSegment::new(StructuralDefinitionSiteRole::Lambda, ordinal),
        [],
    )
}

fn identifier(value: &str) -> CanonicalIdentifier {
    CanonicalIdentifier::new(value).unwrap()
}

fn empty_interface() -> CrossConeHirInterfaceSectionV1 {
    CrossConeHirInterfaceSectionV1::new(
        CanonicalPublicExportBindingsV1::try_new(Vec::new()).unwrap(),
        CanonicalNominalInterfacesV1::try_new(Vec::new()).unwrap(),
        CanonicalCallableInterfacesV1::try_new(Vec::new()).unwrap(),
        CanonicalPropertyInterfacesV1::try_new(Vec::new()).unwrap(),
        CanonicalTypeAliasInterfacesV1::try_new(Vec::new()).unwrap(),
        CanonicalCallableSourceInterfacesV1::try_new(Vec::new()).unwrap(),
        CanonicalExportDefaultTemplatesV1::try_new(Vec::new()).unwrap(),
        CanonicalExportConstValuesV1::try_new(Vec::new()).unwrap(),
        CanonicalExportDefinitionSourcesV1::try_new(Vec::new()).unwrap(),
        CanonicalExternalHirReferencesV1::try_new(Vec::new()).unwrap(),
    )
}

fn cone(name: &str) -> ConeIdentity {
    ConeCoordinate::new("test", name, "1.0.0")
        .unwrap()
        .identity()
        .unwrap()
}
