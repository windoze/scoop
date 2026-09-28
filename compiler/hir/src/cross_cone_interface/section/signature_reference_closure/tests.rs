use crate::{
    PropertyAccessorImplementationV1 as AccessorForm, PropertyAccessorSourceV1 as AccessorSource,
    PropertyAccessorsV1 as Accessors,
};

use std::collections::BTreeMap;

use scoop_identity::{
    AccessorRole, BindingTarget, CallableTemplateOrigin, CallingConvention, CanonicalIdentifier,
    ConeCoordinate, ConeIdentity, DeclarationScope, DefinitionOrigin, DefinitionOwnerChain, Effect,
    EnumVariantFieldKey, EnumVariantFieldSelector, EnumVariantIdentityKey, FieldIdentityKey,
    GcEffect, NominalDeclarationOwner, NonEmptyVec, NormalizedSourcePath, PackagePath,
    PersistentEnumVariantFieldId, PersistentEnumVariantId, PersistentExportBindingId,
    PersistentFieldId, PersistentGenericFunctionId, PersistentGenericTypeId,
    PersistentPropertyAccessorId, PersistentTypeAliasId, PersistentTypeId, PropertyAccessorKey,
    PropertyOwner, SignatureTypeKey, SourceContextKey, SourceDeclarationKey, SourceDeclarationSite,
    SourceIdentity, SourceNominalKind, SourceSpan,
};
use scoop_wire::WirePath;

use super::*;
use crate::{
    CallableImplementationV1, CallableInfixV1, CallableInterfaceRecordV1, CallableModalityV1,
    CallableOperatorRoleV1, CallableParameterCallingV1, CallableSafetyV1, CallableSourceEffectsV1,
    CallableSourceInterfaceV1, CallableSourceParameterV1, CanonicalBinderListV1,
    CanonicalCallableInterfacesV1, CanonicalCallableSourceInterfacesV1,
    CanonicalCallableSourceParametersV1, CanonicalDependencyBindingWitnessesV1,
    CanonicalExportConstValuesV1, CanonicalExportDefaultTemplatesV1,
    CanonicalExportDefinitionSourcesV1, CanonicalExternalHirReferenceRolesV1,
    CanonicalExternalHirReferencesV1, CanonicalNominalInterfacesV1, CanonicalPersistentIdsV1,
    CanonicalPropertyInterfacesV1, CanonicalPublicExportBindingsV1, CanonicalPublicMemberRefsV1,
    CanonicalSignatureTypesV1, CanonicalTypeAliasInterfacesV1, EnumSourceFieldV1,
    EnumSourceShapeV1, EnumSourceVariantStyleV1, EnumSourceVariantV1, ExportDefinitionSourceV1,
    ExternalHirReferenceRoleV1, ExternalHirReferenceSemanticAuthority, ExternalHirReferenceV1,
    ExternalHirTargetV1, NominalSourceFieldV1, NominalSourceShapeV1, NominalTypeParameterBoundsV1,
    PropertyInterfaceRecordV1, PropertyPublicAccessV1, PropertyRepresentationV1,
    PublicDeclarationOwnerV1, PublicExportBindingClosureAuthority, PublicLookupAccessV1,
    PublicNominalKindV1, SourceParameterShapeV1, StructSourceShapeV1, TypeParameterBinderV1,
    TypeParameterBoundLocation, TypeParameterBoundsV1,
};

#[test]
fn accepts_the_exact_foreign_nominal_leaf_union_from_all_signature_fields() {
    let fixture = Fixture::new();

    assert_eq!(fixture.validate(&fixture.section), Ok(()));
    assert_eq!(
        fixture.section.external_references().records().len(),
        fixture.cases.len()
    );
}

#[test]
fn every_signature_carrier_requires_its_external_reference() {
    let fixture = Fixture::new();

    for case in &fixture.cases {
        let section = fixture.with_references(
            fixture
                .section
                .external_references()
                .records()
                .iter()
                .filter(|record| record.target() != case.target)
                .cloned()
                .collect(),
        );

        assert_eq!(
            fixture.validate(&section),
            Err(
                ExternalHirSignatureClosureValidationError::MissingReference {
                    site: case.site,
                    target: case.target,
                }
            )
        );
    }
}

#[test]
fn rejects_missing_role_and_wrong_origin_at_the_exact_use_site() {
    let fixture = Fixture::new();
    let case = fixture.case(ExternalHirSignatureUseSiteV1::CallableResult { record_index: 0 });

    let missing_role = fixture.replace_reference(
        case.target,
        ExternalHirReferenceRoleV1::ConstType,
        fixture.provider,
    );
    let record_index = missing_role
        .external_references()
        .records()
        .binary_search_by_key(&case.target, ExternalHirReferenceV1::target)
        .unwrap();
    assert_eq!(
        fixture.validate(&missing_role),
        Err(ExternalHirSignatureClosureValidationError::MissingRole {
            site: case.site,
            record_index,
            target: case.target,
        })
    );

    let wrong_origin = fixture.replace_reference(
        case.target,
        ExternalHirReferenceRoleV1::SignatureDependency,
        fixture.alternate,
    );
    assert_eq!(
        fixture.validate(&wrong_origin),
        Err(ExternalHirSignatureClosureValidationError::OriginMismatch(
            Box::new(ExternalHirSignatureOriginMismatch {
                site: case.site,
                record_index,
                target: case.target,
                expected: fixture.provider,
                actual: fixture.alternate,
            })
        ))
    );
}

#[test]
fn rejects_unobserved_signature_roles_and_origin_authority_failures() {
    let fixture = Fixture::new();
    let extra_target = ExternalHirTargetV1::TypeAlias(type_alias(fixture.provider, "Extra"));
    let mut extra = fixture.with_references(vec![reference(
        fixture.provider,
        extra_target,
        ExternalHirReferenceRoleV1::SignatureDependency,
    )]);
    extra.nominal_interfaces = CanonicalNominalInterfacesV1::try_new(Vec::new()).unwrap();
    extra.callable_interfaces = CanonicalCallableInterfacesV1::try_new(Vec::new()).unwrap();
    extra.property_interfaces = CanonicalPropertyInterfacesV1::try_new(Vec::new()).unwrap();
    extra.source_interfaces = CanonicalCallableSourceInterfacesV1::try_new(Vec::new()).unwrap();
    assert_eq!(
        fixture.validate(&extra),
        Err(ExternalHirSignatureClosureValidationError::ExtraRole {
            record_index: 0,
            target: extra_target,
        })
    );

    let first = fixture.cases.first().copied().unwrap();
    let mut authority = fixture.authority.clone();
    authority.origins.remove(&first.target);
    assert_eq!(
        fixture.validate_with(&fixture.section, &mut authority),
        Err(ExternalHirSignatureClosureValidationError::TargetOrigin {
            site: first.site,
            target: first.target,
            error: AuthorityError,
        })
    );
}

#[test]
fn ignores_current_cone_nominals_nested_inside_a_foreign_application() {
    let fixture = Fixture::new();
    let local_target =
        ExternalHirTargetV1::Nominal(NominalDeclarationOwner::Concrete(fixture.local_argument));

    assert!(
        !fixture
            .section
            .external_references()
            .records()
            .iter()
            .any(|record| record.target() == local_target)
    );
    assert_eq!(fixture.authority.origins[&local_target], fixture.current);
    assert_eq!(fixture.validate(&fixture.section), Ok(()));
}

#[derive(Clone, Copy)]
struct Case {
    target: ExternalHirTargetV1,
    site: ExternalHirSignatureUseSiteV1,
}

struct Fixture {
    current: ConeIdentity,
    provider: ConeIdentity,
    alternate: ConeIdentity,
    local_argument: PersistentTypeId,
    section: CrossConeHirInterfaceSectionV1,
    cases: Vec<Case>,
    authority: Authority,
}

impl Fixture {
    fn new() -> Self {
        let current = cone("current");
        let provider = cone("provider");
        let alternate = cone("alternate");

        let nominal_bound = concrete_target(provider, "NominalBound");
        let nominal_supertype = generic_target(provider, "NominalSupertype");
        let struct_field_type = concrete_target(provider, "StructFieldType");
        let enum_field_type = concrete_target(provider, "EnumFieldType");
        let callable_bound = concrete_target(provider, "CallableBound");
        let callable_receiver = concrete_target(provider, "CallableReceiver");
        let callable_parameter = concrete_target(provider, "CallableParameter");
        let callable_result = concrete_target(provider, "CallableResult");
        let property_bound = concrete_target(provider, "PropertyBound");
        let property_receiver = concrete_target(provider, "PropertyReceiver");
        let property_value = concrete_target(provider, "PropertyValue");
        let source_parameter = concrete_target(provider, "SourceParameter");
        let source_vararg_value = concrete_target(provider, "SourceVarargValue");
        let source_vararg_element = concrete_target(provider, "SourceVarargElement");
        let local_argument = concrete_id(current, "LocalArgument");

        let (nominals, struct_declaration, enum_declaration) = nominal_interfaces(
            current,
            nominal_bound,
            nominal_supertype,
            local_argument,
            struct_field_type,
            enum_field_type,
        );
        let struct_index = nominal_index(&nominals, struct_declaration);
        let enum_index = nominal_index(&nominals, enum_declaration);

        let (callables, callable) = callable_interfaces(
            current,
            callable_bound,
            callable_receiver,
            callable_parameter,
            callable_result,
        );
        let properties =
            property_interfaces(current, property_bound, property_receiver, property_value);
        let source_interfaces = source_interfaces(
            callable,
            current,
            source_parameter,
            source_vararg_value,
            source_vararg_element,
        );

        let cases = vec![
            Case {
                target: nominal_bound,
                site: ExternalHirSignatureUseSiteV1::NominalTypeParameter {
                    record_index: struct_index,
                    binder_index: 0,
                    bound: TypeParameterBoundLocation::Class,
                },
            },
            Case {
                target: nominal_supertype,
                site: ExternalHirSignatureUseSiteV1::NominalSupertype {
                    record_index: struct_index,
                    signature_index: 0,
                },
            },
            Case {
                target: struct_field_type,
                site: ExternalHirSignatureUseSiteV1::NominalField {
                    record_index: struct_index,
                    field_index: 0,
                },
            },
            Case {
                target: enum_field_type,
                site: ExternalHirSignatureUseSiteV1::NominalEnumField {
                    record_index: enum_index,
                    variant_index: 0,
                    field_index: 0,
                },
            },
            Case {
                target: callable_bound,
                site: ExternalHirSignatureUseSiteV1::CallableTypeParameter {
                    record_index: 0,
                    binder_index: 0,
                    bound: TypeParameterBoundLocation::Interface { interface_index: 0 },
                },
            },
            Case {
                target: callable_receiver,
                site: ExternalHirSignatureUseSiteV1::CallableReceiver { record_index: 0 },
            },
            Case {
                target: callable_parameter,
                site: ExternalHirSignatureUseSiteV1::CallableParameter {
                    record_index: 0,
                    parameter_index: 0,
                },
            },
            Case {
                target: callable_result,
                site: ExternalHirSignatureUseSiteV1::CallableResult { record_index: 0 },
            },
            Case {
                target: property_bound,
                site: ExternalHirSignatureUseSiteV1::PropertyTypeParameter {
                    record_index: 0,
                    binder_index: 0,
                    bound: TypeParameterBoundLocation::Class,
                },
            },
            Case {
                target: property_receiver,
                site: ExternalHirSignatureUseSiteV1::PropertyReceiver { record_index: 0 },
            },
            Case {
                target: property_value,
                site: ExternalHirSignatureUseSiteV1::PropertyValue { record_index: 0 },
            },
            Case {
                target: source_parameter,
                site: ExternalHirSignatureUseSiteV1::SourceParameter {
                    record_index: 0,
                    parameter_index: 0,
                },
            },
            Case {
                target: source_vararg_value,
                site: ExternalHirSignatureUseSiteV1::SourceParameter {
                    record_index: 0,
                    parameter_index: 1,
                },
            },
            Case {
                target: source_vararg_element,
                site: ExternalHirSignatureUseSiteV1::SourceVarargElement {
                    record_index: 0,
                    parameter_index: 1,
                },
            },
        ];
        let references = references(
            cases
                .iter()
                .map(|case| {
                    reference(
                        provider,
                        case.target,
                        ExternalHirReferenceRoleV1::SignatureDependency,
                    )
                })
                .collect(),
        );
        let section = CrossConeHirInterfaceSectionV1::new(
            CanonicalPublicExportBindingsV1::try_new(Vec::new()).unwrap(),
            nominals,
            callables,
            properties,
            CanonicalTypeAliasInterfacesV1::try_new(Vec::new()).unwrap(),
            source_interfaces,
            CanonicalExportDefaultTemplatesV1::try_new(Vec::new()).unwrap(),
            CanonicalExportConstValuesV1::try_new(Vec::new()).unwrap(),
            CanonicalExportDefinitionSourcesV1::try_new(Vec::new()).unwrap(),
            references,
            Default::default(),
            Default::default(),
            Default::default(),
        );

        let mut origins = BTreeMap::new();
        origins.insert(
            ExternalHirTargetV1::Nominal(NominalDeclarationOwner::Concrete(local_argument)),
            current,
        );
        for case in &cases {
            origins.insert(case.target, provider);
        }

        Self {
            current,
            provider,
            alternate,
            local_argument,
            section,
            cases,
            authority: Authority { current, origins },
        }
    }

    fn case(&self, site: ExternalHirSignatureUseSiteV1) -> Case {
        self.cases
            .iter()
            .copied()
            .find(|case| case.site == site)
            .unwrap()
    }

    fn with_references(
        &self,
        records: Vec<ExternalHirReferenceV1>,
    ) -> CrossConeHirInterfaceSectionV1 {
        let mut section = self.section.clone();
        section.external_references = references(records);
        section
    }

    fn replace_reference(
        &self,
        target: ExternalHirTargetV1,
        role: ExternalHirReferenceRoleV1,
        origin: ConeIdentity,
    ) -> CrossConeHirInterfaceSectionV1 {
        self.with_references(
            self.section
                .external_references()
                .records()
                .iter()
                .map(|record| {
                    if record.target() == target {
                        reference(origin, target, role)
                    } else {
                        record.clone()
                    }
                })
                .collect(),
        )
    }

    fn validate(
        &self,
        section: &CrossConeHirInterfaceSectionV1,
    ) -> Result<(), ExternalHirSignatureClosureValidationError<AuthorityError>> {
        self.validate_with(section, &mut self.authority.clone())
    }

    fn validate_with(
        &self,
        section: &CrossConeHirInterfaceSectionV1,
        authority: &mut Authority,
    ) -> Result<(), ExternalHirSignatureClosureValidationError<AuthorityError>> {
        section.validate_signature_reference_closure(authority, &WirePath::root())
    }
}

fn nominal_interfaces(
    current: ConeIdentity,
    nominal_bound: ExternalHirTargetV1,
    nominal_supertype: ExternalHirTargetV1,
    local_argument: PersistentTypeId,
    struct_field_type: ExternalHirTargetV1,
    enum_field_type: ExternalHirTargetV1,
) -> (
    CanonicalNominalInterfacesV1,
    NominalDeclarationOwner,
    NominalDeclarationOwner,
) {
    let structure_key = SourceDeclarationKey::nominal(
        site(current),
        identifier("HostStruct"),
        SourceNominalKind::Struct,
        1,
    );
    let structure = NominalDeclarationOwner::GenericTemplate(
        PersistentGenericTypeId::from_source_declaration(&structure_key).unwrap(),
    );
    let field = PersistentFieldId::from_key(
        &FieldIdentityKey::source_declared(&structure_key, identifier("value")).unwrap(),
    )
    .unwrap();
    let structure_record = crate::nominal_interface_fixture::public_record(
        structure,
        PublicNominalKindV1::Struct,
        binder_list(
            "T",
            TypeParameterBoundsV1::Nominal(
                NominalTypeParameterBoundsV1::try_new(
                    Some(signature(nominal_bound)),
                    CanonicalSignatureTypesV1::try_new(Vec::new()).unwrap(),
                )
                .unwrap(),
            ),
        ),
        CanonicalSignatureTypesV1::try_new(vec![SignatureTypeKey::NominalApplication {
            origin: generic_id(nominal_supertype),
            arguments: NonEmptyVec::new(vec![SignatureTypeKey::Nominal(local_argument)]).unwrap(),
        }])
        .unwrap(),
        CanonicalPersistentIdsV1::try_new(Vec::new()).unwrap(),
        CanonicalPublicMemberRefsV1::try_new(Vec::new()).unwrap(),
        CanonicalPersistentIdsV1::try_new(Vec::new()).unwrap(),
        NominalSourceShapeV1::Struct(
            StructSourceShapeV1::try_new(
                vec![NominalSourceFieldV1::new(
                    field,
                    signature(struct_field_type),
                )],
                crate::NominalCLayoutPolicyV1::Ordinary,
                false,
            )
            .unwrap(),
        ),
    )
    .unwrap();

    let enumeration_key = SourceDeclarationKey::nominal(
        site(current),
        identifier("HostEnum"),
        SourceNominalKind::Enum,
        0,
    );
    let enumeration = NominalDeclarationOwner::Concrete(
        PersistentTypeId::from_source_declaration(&enumeration_key).unwrap(),
    );
    let variant_key =
        EnumVariantIdentityKey::source(&enumeration_key, identifier("Value")).unwrap();
    let variant = PersistentEnumVariantId::from_key(&variant_key).unwrap();
    let enum_field = PersistentEnumVariantFieldId::from_key(&EnumVariantFieldKey::new(
        variant,
        EnumVariantFieldSelector::Positional {
            declaration_index: 0,
        },
    ))
    .unwrap();
    let enumeration_record = crate::nominal_interface_fixture::public_record(
        enumeration,
        PublicNominalKindV1::Enum,
        empty_binders(),
        CanonicalSignatureTypesV1::try_new(Vec::new()).unwrap(),
        CanonicalPersistentIdsV1::try_new(Vec::new()).unwrap(),
        CanonicalPublicMemberRefsV1::try_new(Vec::new()).unwrap(),
        CanonicalPersistentIdsV1::try_new(Vec::new()).unwrap(),
        NominalSourceShapeV1::Enum(
            EnumSourceShapeV1::try_new(vec![
                EnumSourceVariantV1::try_new(
                    variant,
                    EnumSourceVariantStyleV1::Positional,
                    vec![EnumSourceFieldV1::new(
                        enum_field,
                        signature(enum_field_type),
                    )],
                )
                .unwrap(),
            ])
            .unwrap(),
        ),
    )
    .unwrap();

    (
        CanonicalNominalInterfacesV1::try_new(vec![structure_record, enumeration_record]).unwrap(),
        structure,
        enumeration,
    )
}

fn callable_interfaces(
    current: ConeIdentity,
    bound: ExternalHirTargetV1,
    receiver: ExternalHirTargetV1,
    parameter: ExternalHirTargetV1,
    result: ExternalHirTargetV1,
) -> (CanonicalCallableInterfacesV1, CallableTemplateOrigin) {
    let declaration_key = SourceDeclarationKey::function(
        site(current),
        identifier("invoke"),
        1,
        Some(signature(receiver)),
        vec![signature(parameter)],
    );
    let declaration = CallableTemplateOrigin::GenericFunction(
        PersistentGenericFunctionId::from_source_declaration(&declaration_key).unwrap(),
    );
    let record = CallableInterfaceRecordV1::try_new(
        declaration,
        PublicDeclarationOwnerV1::Extension,
        binder_list(
            "T",
            TypeParameterBoundsV1::Nominal(
                NominalTypeParameterBoundsV1::try_new(
                    None,
                    CanonicalSignatureTypesV1::try_new(vec![signature(bound)]).unwrap(),
                )
                .unwrap(),
            ),
        ),
        Some(signature(receiver)),
        crate::CanonicalSourceParameterShapesV1::try_new(vec![SourceParameterShapeV1::new(
            identifier("argument"),
            signature(parameter),
        )])
        .unwrap(),
        SignatureTypeKey::RawPointer(Box::new(signature(result))),
        effects(),
        CallableModalityV1::Final,
        PublicLookupAccessV1::DirectOnly,
        crate::CanonicalPersistentIdsV1::empty(),
    )
    .unwrap();
    (
        CanonicalCallableInterfacesV1::try_new(vec![record]).unwrap(),
        declaration,
    )
}

fn property_interfaces(
    current: ConeIdentity,
    bound: ExternalHirTargetV1,
    receiver: ExternalHirTargetV1,
    value: ExternalHirTargetV1,
) -> CanonicalPropertyInterfacesV1 {
    let declaration_key = SourceDeclarationKey::extension_property(
        site(current),
        identifier("item"),
        1,
        signature(receiver),
    );
    let property =
        scoop_identity::PersistentExtensionPropertyId::from_source_declaration(&declaration_key)
            .unwrap();
    let owner = PropertyOwner::ExtensionProperty(property);
    let getter = PersistentPropertyAccessorId::from_key(&PropertyAccessorKey::new(
        owner,
        AccessorRole::Getter,
    ))
    .unwrap();
    let record = PropertyInterfaceRecordV1::try_new(
        owner,
        PublicDeclarationOwnerV1::Extension,
        binder_list(
            "P",
            TypeParameterBoundsV1::Nominal(
                NominalTypeParameterBoundsV1::try_new(
                    Some(signature(bound)),
                    CanonicalSignatureTypesV1::try_new(Vec::new()).unwrap(),
                )
                .unwrap(),
            ),
        ),
        Some(signature(receiver)),
        signature(value),
        Accessors::read_only(AccessorSource::new(getter, AccessorForm::Body)),
        PropertyRepresentationV1::RuntimeAccessor,
        PropertyPublicAccessV1::DirectOnly,
        crate::PropertySetterPublicAccessV1::Restricted,
    )
    .unwrap();
    CanonicalPropertyInterfacesV1::try_new(vec![record]).unwrap()
}

fn source_interfaces(
    owner: CallableTemplateOrigin,
    current: ConeIdentity,
    parameter: ExternalHirTargetV1,
    vararg_value: ExternalHirTargetV1,
    vararg_element: ExternalHirTargetV1,
) -> CanonicalCallableSourceInterfacesV1 {
    CanonicalCallableSourceInterfacesV1::try_new(vec![
        CallableSourceInterfaceV1::try_new(
            owner,
            CanonicalCallableSourceParametersV1::try_new(vec![
                CallableSourceParameterV1::new(
                    identifier("source"),
                    SignatureTypeKey::Tuple(NonEmptyVec::new(vec![signature(parameter)]).unwrap()),
                    CallableParameterCallingV1::Required,
                    definition_origin(current, 1),
                ),
                CallableSourceParameterV1::new(
                    identifier("items"),
                    signature(vararg_value),
                    CallableParameterCallingV1::VarargEmpty {
                        element_type: SignatureTypeKey::NativeFunctionPointer {
                            calling_convention: CallingConvention::C,
                            parameters: Vec::new(),
                            result: Box::new(signature(vararg_element)),
                        },
                    },
                    definition_origin(current, 2),
                ),
            ])
            .unwrap(),
        )
        .unwrap(),
    ])
    .unwrap()
}

fn binder_list(name: &str, bounds: TypeParameterBoundsV1) -> CanonicalBinderListV1 {
    CanonicalBinderListV1::try_new(vec![TypeParameterBinderV1::new(identifier(name), bounds)])
        .unwrap()
}

fn empty_binders() -> CanonicalBinderListV1 {
    CanonicalBinderListV1::try_new(Vec::new()).unwrap()
}

fn effects() -> CallableSourceEffectsV1 {
    CallableSourceEffectsV1::try_new(
        Effect::Ordinary,
        CallableSafetyV1::Safe,
        GcEffect::Managed,
        CallableImplementationV1::Scoop,
        CallableOperatorRoleV1::None,
        CallableInfixV1::Ordinary,
    )
    .unwrap()
}

fn references(records: Vec<ExternalHirReferenceV1>) -> CanonicalExternalHirReferencesV1 {
    CanonicalExternalHirReferencesV1::try_new(records).unwrap()
}

fn reference(
    origin: ConeIdentity,
    target: ExternalHirTargetV1,
    role: ExternalHirReferenceRoleV1,
) -> ExternalHirReferenceV1 {
    ExternalHirReferenceV1::try_new(
        origin,
        target,
        CanonicalExternalHirReferenceRolesV1::try_new(vec![role]).unwrap(),
        CanonicalDependencyBindingWitnessesV1::try_new(Vec::new()).unwrap(),
        Default::default(),
        Default::default(),
    )
    .unwrap()
}

fn signature(target: ExternalHirTargetV1) -> SignatureTypeKey {
    match target {
        ExternalHirTargetV1::Nominal(NominalDeclarationOwner::Concrete(declaration)) => {
            SignatureTypeKey::Nominal(declaration)
        }
        ExternalHirTargetV1::Nominal(NominalDeclarationOwner::GenericTemplate(declaration)) => {
            SignatureTypeKey::NominalApplication {
                origin: declaration,
                arguments: NonEmptyVec::new(vec![SignatureTypeKey::Binder { depth: 0, index: 0 }])
                    .unwrap(),
            }
        }
        target => panic!("test target {target:?} is not nominal"),
    }
}

fn generic_id(target: ExternalHirTargetV1) -> PersistentGenericTypeId {
    match target {
        ExternalHirTargetV1::Nominal(NominalDeclarationOwner::GenericTemplate(declaration)) => {
            declaration
        }
        target => panic!("test target {target:?} is not a generic nominal"),
    }
}

fn concrete_target(origin: ConeIdentity, name: &str) -> ExternalHirTargetV1 {
    ExternalHirTargetV1::Nominal(NominalDeclarationOwner::Concrete(concrete_id(origin, name)))
}

fn generic_target(origin: ConeIdentity, name: &str) -> ExternalHirTargetV1 {
    let key =
        SourceDeclarationKey::nominal(site(origin), identifier(name), SourceNominalKind::Class, 1);
    ExternalHirTargetV1::Nominal(NominalDeclarationOwner::GenericTemplate(
        PersistentGenericTypeId::from_source_declaration(&key).unwrap(),
    ))
}

fn concrete_id(origin: ConeIdentity, name: &str) -> PersistentTypeId {
    let key =
        SourceDeclarationKey::nominal(site(origin), identifier(name), SourceNominalKind::Class, 0);
    PersistentTypeId::from_source_declaration(&key).unwrap()
}

fn type_alias(origin: ConeIdentity, name: &str) -> PersistentTypeAliasId {
    PersistentTypeAliasId::from_source_declaration(&SourceDeclarationKey::type_alias(
        site(origin),
        identifier(name),
    ))
    .unwrap()
}

fn nominal_index(
    interfaces: &CanonicalNominalInterfacesV1,
    declaration: NominalDeclarationOwner,
) -> usize {
    interfaces
        .records()
        .iter()
        .position(|record| record.declaration() == declaration)
        .unwrap()
}

fn cone(name: &str) -> ConeIdentity {
    ConeCoordinate::new("example", name, "1.0.0")
        .unwrap()
        .identity()
        .unwrap()
}

fn site(origin: ConeIdentity) -> SourceDeclarationSite {
    SourceDeclarationSite::new(
        origin,
        PackagePath::root(),
        DefinitionOwnerChain::top_level(),
        DeclarationScope::ConeWide,
    )
    .unwrap()
}

fn identifier(value: &str) -> CanonicalIdentifier {
    CanonicalIdentifier::new(value).unwrap()
}

fn definition_origin(cone: ConeIdentity, point: u64) -> ExportDefinitionSourceV1 {
    let source =
        SourceIdentity::new(cone, NormalizedSourcePath::new("main.scoop").unwrap()).unwrap();
    let context = SourceContextKey::File {
        source: source.clone(),
    };
    ExportDefinitionSourceV1::new(
        DefinitionOrigin::new(source, SourceSpan::new(point, point + 1).unwrap(), &context)
            .unwrap(),
    )
}

#[derive(Clone)]
struct Authority {
    current: ConeIdentity,
    origins: BTreeMap<ExternalHirTargetV1, ConeIdentity>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct AuthorityError;

impl fmt::Display for AuthorityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("missing test authority")
    }
}

impl std::error::Error for AuthorityError {}

impl ExternalHirReferenceSemanticAuthority<AuthorityError> for Authority {
    fn current_cone(&self) -> ConeIdentity {
        self.current
    }

    fn external_hir_target_origin(
        &mut self,
        target: ExternalHirTargetV1,
    ) -> Result<ConeIdentity, AuthorityError> {
        self.origins.get(&target).copied().ok_or(AuthorityError)
    }

    fn external_hir_target_binding_root(
        &mut self,
        _target: ExternalHirTargetV1,
    ) -> Result<BindingTarget, AuthorityError> {
        Err(AuthorityError)
    }
}

impl PublicExportBindingClosureAuthority for Authority {
    fn closure_node_count(&self) -> usize {
        0
    }

    fn is_direct_dependency(&self, _provider: ConeIdentity) -> bool {
        false
    }

    fn binding_key(
        &self,
        _binding: PersistentExportBindingId,
    ) -> Option<&scoop_identity::ExportBindingKey> {
        None
    }

    fn public_bindings(&self, _exporter: ConeIdentity) -> Option<&CanonicalPublicExportBindingsV1> {
        None
    }
}
