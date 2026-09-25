use std::{collections::BTreeMap, fmt};

use scoop_identity::{
    AccessorRole, BindingTarget, CallableTemplateOrigin, CanonicalIdentifier, CborIdentityRecord,
    ConeCoordinate, ConeIdentity, DeclarationScope, DefinitionOrigin, DefinitionOwnerAtom,
    DefinitionOwnerChain, ExportBindingKey, FieldIdentityKey, GeneratedCallableKey,
    LexicalCallableParent, LexicalCallableRole, NominalDeclarationOwner, NonEmptyVec,
    NormalizedSourcePath, OptionalSignatureType, PackagePath, PersistentConstructorId,
    PersistentEnumVariantId, PersistentExportBindingId, PersistentFieldId, PersistentFunctionId,
    PersistentGeneratedCallableId, PersistentGenericFunctionId, PersistentObjectValueId,
    PersistentPropertyAccessorId, PersistentPropertyId, PersistentTypeAliasId, PersistentTypeId,
    PropertyAccessorKey, PropertyOwner, SignatureTypeKey, SourceContextKey, SourceDeclarationKey,
    SourceDeclarationSite, SourceIdentity, SourceNominalKind, SourceSpan, StructuralDefinitionPath,
    StructuralDefinitionSiteRole, StructuralPathSegment,
};
use scoop_wire::WirePath;

use super::*;
use crate::{
    CanonicalBinderUseListV1, CanonicalBooleanV1, CanonicalCallableInterfacesV1,
    CanonicalCallableSourceInterfacesV1, CanonicalDependencyBindingWitnessesV1,
    CanonicalExportConstValuesV1, CanonicalExportDefaultTemplatesV1,
    CanonicalExportDefinitionSourcesV1, CanonicalExternalHirReferenceRolesV1,
    CanonicalExternalHirReferencesV1, CanonicalNominalInterfacesV1, CanonicalPropertyInterfacesV1,
    CanonicalPublicExportBindingsV1, CanonicalTemplateLocalTableV1,
    CanonicalTemplateValueParametersV1, CanonicalTypeAliasInterfacesV1, DefaultBinderRefV1,
    DefaultCallableRefV1, DefaultExpressionKindV1, DefaultExpressionV1, DependencyBindingWitnessV1,
    ExportDefaultAccessWitnessV1, ExportDefaultBodyV1, ExportDefaultCallDomainV1,
    ExportDefaultReferenceSetV1, ExportDefaultReferenceV1, ExportDefaultTemplateKeyV1,
    ExportDefaultTemplateV1, ExportDefinitionSourceV1, ExternalHirReferenceRoleV1,
    ExternalHirReferenceSemanticAuthority, ExternalHirReferenceV1, ExternalHirTargetV1,
    OptionalTemplateReceiverV1, PersistentLexicalRootV1, PublicExportBindingClosureAuthority,
    ReexportRouteHopV1, ReexportRouteV1,
};

#[test]
fn accepts_the_exact_six_domain_foreign_closure_and_ignores_local_leaves() {
    let fixture = Fixture::new();

    assert_eq!(fixture.validate(&fixture.section), Ok(()));
    assert_eq!(
        fixture.section.external_references().records().len(),
        fixture.cases.len()
    );
}

#[test]
fn every_default_reference_domain_requires_its_external_record() {
    let fixture = Fixture::new();

    for case in &fixture.cases {
        let records = fixture
            .section
            .external_references()
            .records()
            .iter()
            .filter(|record| record.target() != case.target)
            .cloned()
            .collect();
        let section = fixture.with_references(records);

        assert_eq!(
            fixture.validate(&section),
            Err(ExternalHirDefaultClosureValidationError::MissingReference {
                site: case.site,
                target: case.target,
            })
        );
    }
}

#[test]
fn rejects_missing_role_and_wrong_origin_at_the_exact_default_use() {
    let fixture = Fixture::new();
    let case = fixture
        .cases
        .iter()
        .find(|case| matches!(case.site, ExternalHirDefaultUseSiteV1::Global { .. }))
        .unwrap();

    let missing_role = fixture.replace_reference(
        case.target,
        fixture.provider,
        ExternalHirReferenceRoleV1::SignatureDependency,
        None,
    );
    let record_index = reference_index(&missing_role, case.target);
    assert_eq!(
        fixture.validate(&missing_role),
        Err(ExternalHirDefaultClosureValidationError::MissingRole {
            site: case.site,
            record_index,
            target: case.target,
        })
    );

    let wrong_origin = fixture.replace_reference(
        case.target,
        fixture.alternate,
        ExternalHirReferenceRoleV1::DefaultDependency,
        Some(fixture.route.clone()),
    );
    let record_index = reference_index(&wrong_origin, case.target);
    assert_eq!(
        fixture.validate(&wrong_origin),
        Err(ExternalHirDefaultClosureValidationError::OriginMismatch(
            Box::new(ExternalHirDefaultOriginMismatch {
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
fn rejects_unobserved_default_roles_and_missing_origin_authority() {
    let fixture = Fixture::new();
    let (extra_alias, _) = type_alias_identity(fixture.provider, "Extra");
    let extra_target = ExternalHirTargetV1::TypeAlias(extra_alias);
    let extra = section(
        CanonicalExportDefaultTemplatesV1::try_new(Vec::new()).unwrap(),
        CanonicalExternalHirReferencesV1::try_new(vec![external_reference(
            fixture.provider,
            extra_target,
            ExternalHirReferenceRoleV1::DefaultDependency,
            Some(fixture.route.clone()),
        )])
        .unwrap(),
    );
    assert_eq!(
        fixture.validate(&extra),
        Err(ExternalHirDefaultClosureValidationError::ExtraRole {
            record_index: 0,
            target: extra_target,
        })
    );

    let case = &fixture.cases[0];
    let mut authority = fixture.authority.clone();
    authority.origins.remove(&case.target);
    assert_eq!(
        fixture.validate_with(&fixture.section, &mut authority),
        Err(ExternalHirDefaultClosureValidationError::TargetOrigin {
            site: case.site,
            target: case.target,
            error: AuthorityError,
        })
    );
}

#[test]
fn maps_all_non_structural_callable_and_constructor_target_forms() {
    let identities = Identities::new();
    let callable = |declaration| {
        DefaultCallableRefV1::try_new(declaration, OptionalSignatureType::Absent, Vec::new())
            .unwrap()
    };

    let declarations = [
        (
            DefaultCallableDeclarationV1::Function(identities.foreign_function),
            ExternalHirTargetV1::Callable(CallableTemplateOrigin::Function(
                identities.foreign_function,
            )),
        ),
        (
            DefaultCallableDeclarationV1::GenericFunction(identities.foreign_generic_function),
            ExternalHirTargetV1::Callable(CallableTemplateOrigin::GenericFunction(
                identities.foreign_generic_function,
            )),
        ),
        (
            DefaultCallableDeclarationV1::PropertyAccessor(identities.foreign_accessor),
            ExternalHirTargetV1::Callable(CallableTemplateOrigin::Accessor(
                identities.foreign_accessor,
            )),
        ),
        (
            DefaultCallableDeclarationV1::Generated(identities.foreign_generated),
            ExternalHirTargetV1::GeneratedCallable(identities.foreign_generated),
        ),
    ];
    for (declaration, expected) in declarations {
        assert_eq!(
            callable_target(&ExportDefaultCallableTargetV1::Callable(callable(
                declaration
            ))),
            Some(expected)
        );
        assert_eq!(
            callable_target(&ExportDefaultCallableTargetV1::FunctionAddress { declaration }),
            Some(expected)
        );
    }

    let class_bound = DefaultBoundCallableRefV1::new(
        DefaultBinderRefV1::new(0, 0),
        DefaultBoundCallableSourceV1::Class {
            bound: signature(identities.foreign_nominal),
            callable: callable(DefaultCallableDeclarationV1::Function(
                identities.foreign_function,
            )),
        },
        signature(identities.foreign_nominal),
    );
    assert_eq!(
        callable_target(&ExportDefaultCallableTargetV1::Bound(class_bound)),
        Some(ExternalHirTargetV1::Callable(
            CallableTemplateOrigin::Function(identities.foreign_function)
        ))
    );

    let interface_member =
        CallableTemplateOrigin::GenericFunction(identities.foreign_generic_function);
    let interface_bound = DefaultBoundCallableRefV1::new(
        DefaultBinderRefV1::new(0, 0),
        DefaultBoundCallableSourceV1::Interface {
            bound: signature(identities.foreign_nominal),
            member: interface_member,
        },
        signature(identities.foreign_nominal),
    );
    assert_eq!(
        callable_target(&ExportDefaultCallableTargetV1::Bound(interface_bound)),
        Some(ExternalHirTargetV1::Callable(interface_member))
    );
    assert_eq!(
        callable_target(&ExportDefaultCallableTargetV1::DerivedEquality {
            owner_type: signature(identities.foreign_nominal),
        }),
        None
    );
    assert_eq!(
        callable_target(&ExportDefaultCallableTargetV1::LocalFunction {
            declaration: CallableTemplateOrigin::Function(identities.foreign_function),
        }),
        Some(ExternalHirTargetV1::Callable(
            CallableTemplateOrigin::Function(identities.foreign_function)
        ))
    );
    for target in [
        ExportDefaultCallableTargetV1::Lambda {
            body: identities.foreign_generated,
        },
        ExportDefaultCallableTargetV1::AnonymousFunction {
            body: identities.foreign_generated,
        },
        ExportDefaultCallableTargetV1::CallableReference {
            invoke: identities.foreign_generated,
        },
    ] {
        assert_eq!(
            callable_target(&target),
            Some(ExternalHirTargetV1::GeneratedCallable(
                identities.foreign_generated
            ))
        );
    }

    assert_eq!(
        constructor_target(&DefaultConstructorRefV1::Class {
            declaration: DefaultClassConstructorIdV1::Generated(identities.foreign_generated),
            owner_type: signature(identities.foreign_nominal),
        }),
        ExternalHirTargetV1::GeneratedCallable(identities.foreign_generated)
    );
    assert_eq!(
        constructor_target(&DefaultConstructorRefV1::Variant {
            declaration: identities.foreign_variant,
            owner_type: signature(identities.foreign_nominal),
        }),
        ExternalHirTargetV1::Callable(CallableTemplateOrigin::VariantConstructor(
            identities.foreign_variant,
        ))
    );
    assert_eq!(
        field_target(&DefaultFieldRefV1::Tuple {
            declaration_index: 0,
        }),
        None
    );
}

#[derive(Clone, Copy)]
struct Case {
    target: ExternalHirTargetV1,
    site: ExternalHirDefaultUseSiteV1,
}

struct Fixture {
    provider: ConeIdentity,
    alternate: ConeIdentity,
    route: ReexportRouteV1,
    section: CrossConeHirInterfaceSectionV1,
    cases: Vec<Case>,
    authority: Authority,
}

impl Fixture {
    fn new() -> Self {
        let identities = Identities::new();
        let current = identities.current;
        let provider = identities.provider;
        let alternate = cone("alternate");
        let owner = CallableTemplateOrigin::Function(identities.current_function);
        let origin = definition_origin(current, 1);
        let foreign_signature = SignatureTypeKey::Tuple(
            NonEmptyVec::new(vec![
                SignatureTypeKey::RawPointer(Box::new(signature(identities.foreign_nominal))),
                signature(identities.local_nominal),
                signature(identities.foreign_nominal),
            ])
            .unwrap(),
        );
        let foreign_callable = DefaultCallableRefV1::try_new(
            DefaultCallableDeclarationV1::Function(identities.foreign_function),
            OptionalSignatureType::Absent,
            Vec::new(),
        )
        .unwrap();
        let local_callable = DefaultCallableRefV1::try_new(
            DefaultCallableDeclarationV1::Function(identities.local_function),
            OptionalSignatureType::Absent,
            Vec::new(),
        )
        .unwrap();
        let foreign_constructor = DefaultConstructorRefV1::Struct {
            declaration: identities.foreign_constructor,
            owner_type: signature(identities.foreign_nominal),
        };
        let foreign_field = DefaultFieldRefV1::Struct {
            declaration: identities.foreign_field,
            owner_type: signature(identities.foreign_nominal),
        };
        let witness =
            ExportDefaultAccessWitnessV1::new(owner, ExportDefaultCallDomainV1::DirectPublic);
        let references = ExportDefaultReferenceSetV1::try_new(
            vec![
                default_reference(
                    ExportDefaultCallableTargetV1::Callable(foreign_callable),
                    &origin,
                    &witness,
                ),
                default_reference(
                    ExportDefaultCallableTargetV1::Callable(local_callable),
                    &origin,
                    &witness,
                ),
                default_reference(
                    ExportDefaultCallableTargetV1::DerivedEquality {
                        owner_type: foreign_signature.clone(),
                    },
                    &origin,
                    &witness,
                ),
            ],
            vec![default_reference(foreign_constructor, &origin, &witness)],
            vec![default_reference(
                foreign_signature.clone(),
                &origin,
                &witness,
            )],
            vec![default_reference(
                identities.foreign_property,
                &origin,
                &witness,
            )],
            vec![default_reference(
                identities.foreign_object,
                &origin,
                &witness,
            )],
            vec![
                default_reference(foreign_field, &origin, &witness),
                default_reference(
                    DefaultFieldRefV1::Tuple {
                        declaration_index: 0,
                    },
                    &origin,
                    &witness,
                ),
            ],
        )
        .unwrap();
        let template = template(owner, foreign_signature, references, origin);
        let templates = CanonicalExportDefaultTemplatesV1::try_new(vec![template]).unwrap();

        let targets = [
            ExternalHirTargetV1::Callable(CallableTemplateOrigin::Function(
                identities.foreign_function,
            )),
            ExternalHirTargetV1::Callable(CallableTemplateOrigin::Constructor(
                identities.foreign_constructor,
            )),
            ExternalHirTargetV1::from(identities.foreign_nominal),
            ExternalHirTargetV1::Property(PropertyOwner::Property(identities.foreign_property)),
            ExternalHirTargetV1::ObjectValue(identities.foreign_object),
            ExternalHirTargetV1::Field(identities.foreign_field),
        ];
        let route = witness_route(provider);
        let external_references = CanonicalExternalHirReferencesV1::try_new(
            targets
                .into_iter()
                .map(|target| {
                    external_reference(
                        provider,
                        target,
                        ExternalHirReferenceRoleV1::DefaultDependency,
                        Some(route.clone()),
                    )
                })
                .collect(),
        )
        .unwrap();
        let section = section(templates, external_references);
        let references = section.default_templates().records()[0].references();
        let cases = vec![
            Case {
                target: targets[0],
                site: ExternalHirDefaultUseSiteV1::Callable {
                    template_index: 0,
                    reference_index: references
                        .callables()
                        .iter()
                        .position(|reference| {
                            callable_target(reference.target()) == Some(targets[0])
                        })
                        .unwrap(),
                },
            },
            Case {
                target: targets[1],
                site: ExternalHirDefaultUseSiteV1::Constructor {
                    template_index: 0,
                    reference_index: 0,
                },
            },
            Case {
                target: targets[2],
                site: ExternalHirDefaultUseSiteV1::Type {
                    template_index: 0,
                    reference_index: 0,
                },
            },
            Case {
                target: targets[3],
                site: ExternalHirDefaultUseSiteV1::Global {
                    template_index: 0,
                    reference_index: 0,
                },
            },
            Case {
                target: targets[4],
                site: ExternalHirDefaultUseSiteV1::Singleton {
                    template_index: 0,
                    reference_index: 0,
                },
            },
            Case {
                target: targets[5],
                site: ExternalHirDefaultUseSiteV1::Field {
                    template_index: 0,
                    reference_index: references
                        .fields()
                        .iter()
                        .position(|reference| field_target(reference.target()) == Some(targets[5]))
                        .unwrap(),
                },
            },
        ];
        let authority = Authority {
            current,
            origins: BTreeMap::from([
                (targets[0], provider),
                (targets[1], provider),
                (targets[2], provider),
                (targets[3], provider),
                (targets[4], provider),
                (targets[5], provider),
                (
                    ExternalHirTargetV1::Callable(CallableTemplateOrigin::Function(
                        identities.local_function,
                    )),
                    current,
                ),
                (ExternalHirTargetV1::from(identities.local_nominal), current),
            ]),
        };
        Self {
            provider,
            alternate,
            route,
            section,
            cases,
            authority,
        }
    }

    fn with_references(
        &self,
        records: Vec<ExternalHirReferenceV1>,
    ) -> CrossConeHirInterfaceSectionV1 {
        section(
            self.section.default_templates().clone(),
            CanonicalExternalHirReferencesV1::try_new(records).unwrap(),
        )
    }

    fn replace_reference(
        &self,
        target: ExternalHirTargetV1,
        origin: ConeIdentity,
        role: ExternalHirReferenceRoleV1,
        route: Option<ReexportRouteV1>,
    ) -> CrossConeHirInterfaceSectionV1 {
        let mut records: Vec<_> = self
            .section
            .external_references()
            .records()
            .iter()
            .filter(|record| record.target() != target)
            .cloned()
            .collect();
        records.push(external_reference(origin, target, role, route));
        self.with_references(records)
    }

    fn validate(
        &self,
        section: &CrossConeHirInterfaceSectionV1,
    ) -> Result<(), ExternalHirDefaultClosureValidationError<AuthorityError>> {
        self.validate_with(section, &mut self.authority.clone())
    }

    fn validate_with(
        &self,
        section: &CrossConeHirInterfaceSectionV1,
        authority: &mut Authority,
    ) -> Result<(), ExternalHirDefaultClosureValidationError<AuthorityError>> {
        section.validate_default_reference_closure(authority, &WirePath::root())
    }
}

struct Identities {
    current: ConeIdentity,
    provider: ConeIdentity,
    current_function: PersistentFunctionId,
    local_function: PersistentFunctionId,
    foreign_function: PersistentFunctionId,
    foreign_generic_function: PersistentGenericFunctionId,
    foreign_nominal: NominalDeclarationOwner,
    local_nominal: NominalDeclarationOwner,
    foreign_constructor: PersistentConstructorId,
    foreign_property: PersistentPropertyId,
    foreign_accessor: PersistentPropertyAccessorId,
    foreign_object: PersistentObjectValueId,
    foreign_field: PersistentFieldId,
    foreign_generated: PersistentGeneratedCallableId,
    foreign_variant: PersistentEnumVariantId,
}

impl Identities {
    fn new() -> Self {
        let current = cone("current");
        let provider = cone("provider");
        let current_function: PersistentFunctionId = function(current, "owner", 0);
        let local_function: PersistentFunctionId = function(current, "local", 0);
        let foreign_function: PersistentFunctionId = function(provider, "foreign", 0);
        let foreign_generic_function: PersistentGenericFunctionId =
            function(provider, "generic", 1);
        let foreign_declaration =
            nominal_declaration(provider, "Foreign", SourceNominalKind::Struct);
        let foreign_type = PersistentTypeId::from_source_declaration(&foreign_declaration).unwrap();
        let foreign_nominal = NominalDeclarationOwner::Concrete(foreign_type);
        let local_declaration = nominal_declaration(current, "Local", SourceNominalKind::Class);
        let local_nominal = NominalDeclarationOwner::Concrete(
            PersistentTypeId::from_source_declaration(&local_declaration).unwrap(),
        );
        let foreign_constructor =
            PersistentConstructorId::from_source_declaration(&SourceDeclarationKey::constructor(
                owned_site(provider, DefinitionOwnerAtom::Type(foreign_type)),
                Vec::new(),
            ))
            .unwrap();
        let property_declaration =
            SourceDeclarationKey::property(site(provider), identifier("foreignProperty"));
        let foreign_property =
            PersistentPropertyId::from_source_declaration(&property_declaration).unwrap();
        let foreign_accessor = PersistentPropertyAccessorId::from_key(&PropertyAccessorKey::new(
            PropertyOwner::Property(foreign_property),
            AccessorRole::Getter,
        ))
        .unwrap();
        let object_declaration =
            nominal_declaration(provider, "ForeignObject", SourceNominalKind::Object);
        let foreign_object =
            PersistentObjectValueId::from_source_object(&object_declaration).unwrap();
        let foreign_field = PersistentFieldId::from_key(
            &FieldIdentityKey::source_declared(&foreign_declaration, identifier("value")).unwrap(),
        )
        .unwrap();
        let foreign_generated =
            PersistentGeneratedCallableId::from_key(&GeneratedCallableKey::Lexical {
                parent: LexicalCallableParent::function(foreign_function),
                role: LexicalCallableRole::LambdaBody,
                path: definition_path(),
            })
            .unwrap();
        let enum_declaration =
            nominal_declaration(provider, "ForeignEnum", SourceNominalKind::Enum);
        let foreign_variant = PersistentEnumVariantId::from_key(
            &scoop_identity::EnumVariantIdentityKey::source(&enum_declaration, identifier("Only"))
                .unwrap(),
        )
        .unwrap();
        Self {
            current,
            provider,
            current_function,
            local_function,
            foreign_function,
            foreign_generic_function,
            foreign_nominal,
            local_nominal,
            foreign_constructor,
            foreign_property,
            foreign_accessor,
            foreign_object,
            foreign_field,
            foreign_generated,
            foreign_variant,
        }
    }
}

fn function<I>(origin: ConeIdentity, name: &str, type_parameter_count: u32) -> I
where
    I: FunctionFromDeclaration,
{
    let declaration = SourceDeclarationKey::function(
        site(origin),
        identifier(name),
        type_parameter_count,
        None,
        Vec::new(),
    );
    I::from_declaration(&declaration)
}

trait FunctionFromDeclaration {
    fn from_declaration(declaration: &SourceDeclarationKey) -> Self;
}

impl FunctionFromDeclaration for PersistentFunctionId {
    fn from_declaration(declaration: &SourceDeclarationKey) -> Self {
        Self::from_source_declaration(declaration).unwrap()
    }
}

impl FunctionFromDeclaration for PersistentGenericFunctionId {
    fn from_declaration(declaration: &SourceDeclarationKey) -> Self {
        Self::from_source_declaration(declaration).unwrap()
    }
}

fn template(
    owner: CallableTemplateOrigin,
    result: SignatureTypeKey,
    references: ExportDefaultReferenceSetV1,
    origin: ExportDefinitionSourceV1,
) -> ExportDefaultTemplateV1 {
    let body = ExportDefaultBodyV1::try_new(
        Vec::new(),
        DefaultExpressionV1::try_new(
            DefaultExpressionKindV1::UnitLiteral,
            result.clone(),
            origin.clone(),
        )
        .unwrap(),
    )
    .unwrap();
    let CallableTemplateOrigin::Function(function) = owner else {
        panic!("test template owner must be a function")
    };
    ExportDefaultTemplateV1::try_new(
        ExportDefaultTemplateKeyV1::new(owner, 0),
        PersistentLexicalRootV1::Function(function),
        definition_path(),
        CanonicalTemplateLocalTableV1::try_new(Vec::new()).unwrap(),
        body,
        result,
        CanonicalBooleanV1::False,
        CanonicalBinderUseListV1::try_new(Vec::new()).unwrap(),
        OptionalTemplateReceiverV1::Absent,
        CanonicalTemplateValueParametersV1::try_new(Vec::new()).unwrap(),
        references,
        origin,
    )
    .unwrap()
}

fn default_reference<T>(
    target: T,
    origin: &ExportDefinitionSourceV1,
    witness: &ExportDefaultAccessWitnessV1,
) -> ExportDefaultReferenceV1<T> {
    ExportDefaultReferenceV1::new(target, origin.clone(), witness.clone())
}

fn external_reference(
    origin: ConeIdentity,
    target: ExternalHirTargetV1,
    role: ExternalHirReferenceRoleV1,
    route: Option<ReexportRouteV1>,
) -> ExternalHirReferenceV1 {
    let witnesses = route
        .into_iter()
        .map(DependencyBindingWitnessV1::new)
        .collect();
    ExternalHirReferenceV1::try_new(
        origin,
        target,
        CanonicalExternalHirReferenceRolesV1::try_new(vec![role]).unwrap(),
        CanonicalDependencyBindingWitnessesV1::try_new(witnesses).unwrap(),
        Default::default(),
        Default::default(),
    )
    .unwrap()
}

fn section(
    templates: CanonicalExportDefaultTemplatesV1,
    references: CanonicalExternalHirReferencesV1,
) -> CrossConeHirInterfaceSectionV1 {
    CrossConeHirInterfaceSectionV1::new(
        CanonicalPublicExportBindingsV1::try_new(Vec::new()).unwrap(),
        CanonicalNominalInterfacesV1::try_new(Vec::new()).unwrap(),
        CanonicalCallableInterfacesV1::try_new(Vec::new()).unwrap(),
        CanonicalPropertyInterfacesV1::try_new(Vec::new()).unwrap(),
        CanonicalTypeAliasInterfacesV1::try_new(Vec::new()).unwrap(),
        CanonicalCallableSourceInterfacesV1::try_new(Vec::new()).unwrap(),
        templates,
        CanonicalExportConstValuesV1::try_new(Vec::new()).unwrap(),
        CanonicalExportDefinitionSourcesV1::try_new(Vec::new()).unwrap(),
        references,
    )
}

fn reference_index(section: &CrossConeHirInterfaceSectionV1, target: ExternalHirTargetV1) -> usize {
    section
        .external_references()
        .records()
        .iter()
        .position(|record| record.target() == target)
        .unwrap()
}

fn signature(target: NominalDeclarationOwner) -> SignatureTypeKey {
    match target {
        NominalDeclarationOwner::Concrete(declaration) => SignatureTypeKey::Nominal(declaration),
        NominalDeclarationOwner::GenericTemplate(_) => {
            panic!("test fixture uses only concrete nominal types")
        }
    }
}

fn nominal_declaration(
    origin: ConeIdentity,
    name: &str,
    kind: SourceNominalKind,
) -> SourceDeclarationKey {
    SourceDeclarationKey::nominal(site(origin), identifier(name), kind, 0)
}

fn type_alias_identity(
    origin: ConeIdentity,
    name: &str,
) -> (PersistentTypeAliasId, PersistentExportBindingId) {
    let declaration = SourceDeclarationKey::type_alias(site(origin), identifier(name));
    let alias = PersistentTypeAliasId::from_source_declaration(&declaration).unwrap();
    let target = BindingTarget::type_alias(&declaration).unwrap();
    (alias, export_binding(origin, name, target))
}

fn witness_route(provider: ConeIdentity) -> ReexportRouteV1 {
    let (_, binding) = type_alias_identity(provider, "WitnessRoot");
    ReexportRouteV1::try_new(provider, vec![ReexportRouteHopV1::new(provider, binding)]).unwrap()
}

fn export_binding(
    provider: ConeIdentity,
    name: &str,
    target: BindingTarget,
) -> PersistentExportBindingId {
    CborIdentityRecord::<PersistentExportBindingId, _>::from_key(ExportBindingKey::new(
        provider,
        PackagePath::root(),
        identifier(name),
        target,
    ))
    .unwrap()
    .id()
}

fn definition_path() -> StructuralDefinitionPath {
    StructuralDefinitionPath::from_first(
        StructuralPathSegment::new(StructuralDefinitionSiteRole::DefaultValue, 0),
        [],
    )
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

fn owned_site(origin: ConeIdentity, owner: DefinitionOwnerAtom) -> SourceDeclarationSite {
    SourceDeclarationSite::new(
        origin,
        PackagePath::root(),
        DefinitionOwnerChain::from_outer_to_inner(vec![owner]),
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

    fn binding_key(&self, _binding: PersistentExportBindingId) -> Option<&ExportBindingKey> {
        None
    }

    fn public_bindings(&self, _exporter: ConeIdentity) -> Option<&CanonicalPublicExportBindingsV1> {
        None
    }
}
