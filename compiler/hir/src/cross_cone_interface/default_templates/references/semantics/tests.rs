use scoop_identity::{
    CallableTemplateOrigin, ConeIdentity, Effect, GcEffect, NominalDeclarationOwner,
    PersistentGenericTypeId, PersistentObjectValueId, PersistentPropertyId, PersistentTypeId,
    SignatureTypeKey, StructuralDefinitionPath, StructuralDefinitionSiteRole,
    StructuralPathSegment,
};
use scoop_wire::WirePath;

use super::*;
mod local_signatures;
mod source_domains;
use crate::cross_cone_interface::default_templates::body::expression_test_support::Fixture;
use crate::{
    CallableImplementationV1, CallableInfixV1, CallableModalityV1, CallableOperatorRoleV1,
    CallableSafetyV1, CallableSourceEffectsV1, CanonicalBinderListV1, CanonicalBinderUseListV1,
    CanonicalBooleanV1, CanonicalSourceParameterShapesV1, CanonicalTemplateLocalTableV1,
    CanonicalTemplateValueParametersV1, DefaultBinderRefV1, DefaultBoundCallableRefV1,
    DefaultBoundCallableSourceV1, DefaultConstructorRefV1, DefaultExpressionKindV1,
    DefaultExpressionV1, DefaultFieldRefV1, ExportDefaultAccessWitnessV1, ExportDefaultBodyV1,
    ExportDefaultCallableReferenceV1, ExportDefaultConstructorReferenceV1,
    ExportDefaultFieldReferenceV1, ExportDefaultGlobalReferenceV1, ExportDefaultReferenceV1,
    ExportDefaultSingletonReferenceV1, ExportDefaultTemplateKeyV1, ExportDefaultTypeReferenceV1,
    NominalInterfaceShapeAuthority, OptionalTemplateReceiverV1, PersistentLexicalRootV1,
    PublicDeclarationOwnerV1, PublicLookupAccessV1, PublicNominalKindV1, PublicNominalShapeV1,
};

#[test]
fn validates_all_six_domains_in_canonical_order() {
    let fixture = Fixture::new();
    let template = template(&fixture, all_references(&fixture));
    let mut authority = Authority::new(&fixture);

    assert_eq!(
        template.validate_reference_envelope_semantics(
            &owner_interface(&fixture, PublicLookupAccessV1::DirectOnly),
            DefaultTemplateProviderShapeV1::try_new(0, 0).unwrap(),
            &mut authority,
            &WirePath::root(),
        ),
        Ok(())
    );
    assert_eq!(
        authority.targets,
        vec![
            ExportDefaultReferenceKindV1::Callable,
            ExportDefaultReferenceKindV1::Constructor,
            ExportDefaultReferenceKindV1::Type,
            ExportDefaultReferenceKindV1::Global,
            ExportDefaultReferenceKindV1::Singleton,
            ExportDefaultReferenceKindV1::Field,
        ]
    );
    assert_eq!(authority.origins, 6);
    assert_eq!(authority.nominals, 3);
}

#[test]
fn maps_owner_public_slot_access_to_the_refined_call_domain() {
    let fixture = Fixture::new();
    let references = reference_set(
        Vec::new(),
        Vec::new(),
        Vec::new(),
        vec![reference(
            fixture.property,
            &fixture,
            ExportDefaultCallDomainV1::DirectAndPublicSlot,
        )],
        Vec::new(),
        Vec::new(),
    );
    let template = template(&fixture, references);
    let mut authority = Authority::new(&fixture);

    assert_eq!(
        template.validate_reference_envelope_semantics(
            &owner_interface(&fixture, PublicLookupAccessV1::PublicSlot),
            DefaultTemplateProviderShapeV1::try_new(0, 0).unwrap(),
            &mut authority,
            &WirePath::root(),
        ),
        Ok(())
    );
}

#[test]
fn rejects_call_domain_before_origin_or_target_authority() {
    let fixture = Fixture::new();
    let references = reference_set(
        Vec::new(),
        Vec::new(),
        Vec::new(),
        vec![reference(
            fixture.property,
            &fixture,
            ExportDefaultCallDomainV1::DirectAndPublicSlot,
        )],
        Vec::new(),
        Vec::new(),
    );
    let template = template(&fixture, references);
    let mut authority = Authority::new(&fixture);

    assert_eq!(
        validate(&template, &fixture, &mut authority),
        Err(ExportDefaultReferenceSetSemanticValidationError::Record {
            kind: ExportDefaultReferenceKindV1::Global,
            index: 0,
            error: Box::new(ExportDefaultReferenceValidationError::CallDomain {
                expected: ExportDefaultCallDomainV1::DirectPublic,
                actual: Some(ExportDefaultCallDomainV1::DirectAndPublicSlot),
            }),
        })
    );
    assert_eq!(authority.origins, 0);
    assert!(authority.targets.is_empty());
}

#[test]
fn rejects_mismatched_owner_interface_and_foreign_reference_origin() {
    let fixture = Fixture::new();
    let references = reference_set(
        Vec::new(),
        Vec::new(),
        Vec::new(),
        vec![reference(
            fixture.property,
            &fixture,
            ExportDefaultCallDomainV1::DirectPublic,
        )],
        Vec::new(),
        Vec::new(),
    );
    let template = template(&fixture, references);
    let mut authority = Authority::new(&fixture);
    let other = CallableInterfaceRecordV1::try_new(
        CallableTemplateOrigin::Constructor(fixture.constructor),
        PublicDeclarationOwnerV1::Nominal(NominalDeclarationOwner::Concrete(fixture.type_id)),
        CanonicalBinderListV1::try_new(Vec::new()).unwrap(),
        None,
        CanonicalSourceParameterShapesV1::try_new(Vec::new()).unwrap(),
        SignatureTypeKey::Nominal(fixture.type_id),
        effects(),
        CallableModalityV1::Final,
        PublicLookupAccessV1::DirectOnly,
        crate::CanonicalPersistentIdsV1::empty(),
    )
    .unwrap();

    assert_eq!(
        template.validate_reference_envelope_semantics(
            &other,
            DefaultTemplateProviderShapeV1::try_new(0, 0).unwrap(),
            &mut authority,
            &WirePath::root(),
        ),
        Err(
            ExportDefaultReferenceSetSemanticValidationError::OwnerInterface {
                expected: CallableTemplateOrigin::Function(fixture.function),
                actual: CallableTemplateOrigin::Constructor(fixture.constructor),
            }
        )
    );
    assert_eq!(authority.origins, 0);

    authority.cone = ConeIdentity::SINGLE_FILE;
    assert_eq!(
        validate(&template, &fixture, &mut authority),
        Err(ExportDefaultReferenceSetSemanticValidationError::Record {
            kind: ExportDefaultReferenceKindV1::Global,
            index: 0,
            error: Box::new(ExportDefaultReferenceValidationError::DefinitionOrigin(
                ExportDefinitionSourceSemanticValidationError::Cone {
                    expected: ConeIdentity::SINGLE_FILE,
                    actual: ConeIdentity::CORE,
                },
            )),
        })
    );
    assert!(authority.targets.is_empty());
}

#[test]
fn rejects_binder_type_targets_and_validates_bound_binders() {
    let fixture = Fixture::new();
    let invalid_type = reference_set(
        Vec::new(),
        Vec::new(),
        vec![reference(
            SignatureTypeKey::Binder { depth: 0, index: 0 },
            &fixture,
            ExportDefaultCallDomainV1::DirectPublic,
        )],
        Vec::new(),
        Vec::new(),
        Vec::new(),
    );
    let invalid_type_template = template(&fixture, invalid_type);
    assert_eq!(
        validate(
            &invalid_type_template,
            &fixture,
            &mut Authority::new(&fixture)
        ),
        Err(ExportDefaultReferenceSetSemanticValidationError::Record {
            kind: ExportDefaultReferenceKindV1::Type,
            index: 0,
            error: Box::new(ExportDefaultReferenceValidationError::BinderTypeTarget {
                depth: 0,
                index: 0,
            }),
        })
    );

    let bound = DefaultBoundCallableRefV1::new(
        DefaultBinderRefV1::new(0, 0),
        DefaultBoundCallableSourceV1::Class {
            bound: SignatureTypeKey::Nominal(fixture.type_id),
            callable: fixture.callable(),
        },
        SignatureTypeKey::Nominal(fixture.type_id),
    );
    let invalid_binder = reference_set(
        vec![reference(
            ExportDefaultCallableTargetV1::Bound(bound),
            &fixture,
            ExportDefaultCallDomainV1::DirectPublic,
        )],
        Vec::new(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
    );
    let invalid_binder_template = template(&fixture, invalid_binder);
    assert!(matches!(
        validate(
            &invalid_binder_template,
            &fixture,
            &mut Authority::new(&fixture)
        ),
        Err(ExportDefaultReferenceSetSemanticValidationError::Record {
            kind: ExportDefaultReferenceKindV1::Callable,
            index: 0,
            error,
        }) if matches!(
            error.as_ref(),
            ExportDefaultReferenceValidationError::Binder {
                site: ExportDefaultReferenceTargetTypeSiteV1::BoundCallableReceiverParameter,
                error: SignatureBinderScopeError::DepthOutOfRange {
                    depth: 0,
                    available_depths: 0,
                },
                ..
            }
        )
    ));
}

#[test]
fn preserves_typed_target_authority_failures() {
    let fixture = Fixture::new();
    let template = template(&fixture, all_references(&fixture));
    let mut authority = Authority::new(&fixture);
    authority.reject = Some(ExportDefaultReferenceKindV1::Field);

    assert_eq!(
        validate(&template, &fixture, &mut authority),
        Err(ExportDefaultReferenceSetSemanticValidationError::Record {
            kind: ExportDefaultReferenceKindV1::Field,
            index: 0,
            error: Box::new(ExportDefaultReferenceValidationError::Target(
                AuthorityError::Rejected(ExportDefaultReferenceKindV1::Field),
            )),
        })
    );
    assert_eq!(authority.origins, 6);
}

fn validate(
    template: &ExportDefaultTemplateV1,
    fixture: &Fixture,
    authority: &mut Authority,
) -> Result<(), ExportDefaultReferenceSetSemanticValidationError<AuthorityError>> {
    template.validate_reference_envelope_semantics(
        &owner_interface(fixture, PublicLookupAccessV1::DirectOnly),
        DefaultTemplateProviderShapeV1::try_new(0, 0).unwrap(),
        authority,
        &WirePath::root(),
    )
}

fn all_references(fixture: &Fixture) -> ExportDefaultReferenceSetV1 {
    let owner_type = SignatureTypeKey::Nominal(fixture.type_id);
    reference_set(
        vec![reference(
            ExportDefaultCallableTargetV1::Callable(fixture.callable()),
            fixture,
            ExportDefaultCallDomainV1::DirectPublic,
        )],
        vec![reference(
            DefaultConstructorRefV1::Struct {
                declaration: fixture.constructor,
                owner_type: owner_type.clone(),
            },
            fixture,
            ExportDefaultCallDomainV1::DirectPublic,
        )],
        vec![reference(
            owner_type.clone(),
            fixture,
            ExportDefaultCallDomainV1::DirectPublic,
        )],
        vec![reference(
            fixture.property,
            fixture,
            ExportDefaultCallDomainV1::DirectPublic,
        )],
        vec![reference(
            fixture.object,
            fixture,
            ExportDefaultCallDomainV1::DirectPublic,
        )],
        vec![reference(
            DefaultFieldRefV1::Struct {
                declaration: fixture.field,
                owner_type,
            },
            fixture,
            ExportDefaultCallDomainV1::DirectPublic,
        )],
    )
}

#[allow(clippy::too_many_arguments)]
fn reference_set(
    callables: Vec<ExportDefaultCallableReferenceV1>,
    constructors: Vec<ExportDefaultConstructorReferenceV1>,
    types: Vec<ExportDefaultTypeReferenceV1>,
    globals: Vec<ExportDefaultGlobalReferenceV1>,
    singleton_values: Vec<ExportDefaultSingletonReferenceV1>,
    fields: Vec<ExportDefaultFieldReferenceV1>,
) -> ExportDefaultReferenceSetV1 {
    ExportDefaultReferenceSetV1::try_new(
        callables,
        constructors,
        types,
        globals,
        singleton_values,
        fields,
    )
    .unwrap()
}

fn reference<T>(
    target: T,
    fixture: &Fixture,
    call_domain: ExportDefaultCallDomainV1,
) -> ExportDefaultReferenceV1<T> {
    ExportDefaultReferenceV1::new(
        target,
        fixture.origin(),
        ExportDefaultAccessWitnessV1::new(
            CallableTemplateOrigin::Function(fixture.function),
            call_domain,
        ),
    )
}

fn template(fixture: &Fixture, references: ExportDefaultReferenceSetV1) -> ExportDefaultTemplateV1 {
    let result = SignatureTypeKey::Nominal(fixture.type_id);
    let body = ExportDefaultBodyV1::try_new(
        Vec::new(),
        DefaultExpressionV1::try_new(
            DefaultExpressionKindV1::UnitLiteral,
            result.clone(),
            fixture.origin(),
        )
        .unwrap(),
    )
    .unwrap();
    ExportDefaultTemplateV1::try_new(
        ExportDefaultTemplateKeyV1::new(CallableTemplateOrigin::Function(fixture.function), 0),
        PersistentLexicalRootV1::Function(fixture.function),
        StructuralDefinitionPath::from_first(
            StructuralPathSegment::new(StructuralDefinitionSiteRole::DefaultValue, 0),
            [],
        ),
        CanonicalTemplateLocalTableV1::try_new(Vec::new()).unwrap(),
        body,
        result,
        CanonicalBooleanV1::False,
        CanonicalBinderUseListV1::try_new(Vec::new()).unwrap(),
        OptionalTemplateReceiverV1::Absent,
        CanonicalTemplateValueParametersV1::try_new(Vec::new()).unwrap(),
        references,
        fixture.origin(),
    )
    .unwrap()
}

fn owner_interface(fixture: &Fixture, access: PublicLookupAccessV1) -> CallableInterfaceRecordV1 {
    let (owner, modality) = match access {
        PublicLookupAccessV1::DirectOnly => (
            PublicDeclarationOwnerV1::TopLevel,
            CallableModalityV1::Final,
        ),
        PublicLookupAccessV1::PublicSlot => (
            PublicDeclarationOwnerV1::Nominal(NominalDeclarationOwner::Concrete(fixture.type_id)),
            CallableModalityV1::Open,
        ),
    };
    CallableInterfaceRecordV1::try_new(
        CallableTemplateOrigin::Function(fixture.function),
        owner,
        CanonicalBinderListV1::try_new(Vec::new()).unwrap(),
        None,
        CanonicalSourceParameterShapesV1::try_new(Vec::new()).unwrap(),
        SignatureTypeKey::Nominal(fixture.type_id),
        effects(),
        modality,
        access,
        crate::CanonicalPersistentIdsV1::try_new(if access == PublicLookupAccessV1::PublicSlot {
            vec![
                scoop_identity::PersistentDispatchSlotId::from_key(
                    &scoop_identity::DispatchSlotKey::virtual_method(fixture.function),
                )
                .unwrap(),
            ]
        } else {
            vec![]
        })
        .unwrap(),
    )
    .unwrap()
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

struct Authority {
    local_keys:
        std::collections::BTreeMap<CallableTemplateOrigin, scoop_identity::SourceDeclarationKey>,
    nominal: PersistentTypeId,
    cone: ConeIdentity,
    targets: Vec<ExportDefaultReferenceKindV1>,
    origins: usize,
    nominals: usize,
    reject: Option<ExportDefaultReferenceKindV1>,
}

impl Authority {
    fn new(fixture: &Fixture) -> Self {
        Self {
            local_keys: Default::default(),
            nominal: fixture.type_id,
            cone: ConeIdentity::CORE,
            targets: Vec::new(),
            origins: 0,
            nominals: 0,
            reject: None,
        }
    }

    fn target(&mut self, kind: ExportDefaultReferenceKindV1) -> Result<(), AuthorityError> {
        self.targets.push(kind);
        if self.reject == Some(kind) {
            Err(AuthorityError::Rejected(kind))
        } else {
            Ok(())
        }
    }
}

impl NominalInterfaceShapeAuthority<AuthorityError> for Authority {
    fn concrete_nominal_shape(
        &mut self,
        declaration: PersistentTypeId,
    ) -> Result<PublicNominalShapeV1, AuthorityError> {
        if declaration != self.nominal {
            return Err(AuthorityError::UnknownNominal);
        }
        self.nominals += 1;
        Ok(PublicNominalShapeV1::new(PublicNominalKindV1::Struct, 0))
    }

    fn generic_nominal_shape(
        &mut self,
        _declaration: PersistentGenericTypeId,
    ) -> Result<PublicNominalShapeV1, AuthorityError> {
        Err(AuthorityError::UnknownNominal)
    }
}

impl ExportDefinitionSourceSemanticAuthority<AuthorityError> for Authority {
    fn current_cone(&self) -> ConeIdentity {
        self.cone
    }

    fn validate_export_definition_source(
        &mut self,
        _source: &ExportDefinitionSourceV1,
    ) -> Result<(), AuthorityError> {
        self.origins += 1;
        Ok(())
    }
}

impl DefaultReferenceSemanticAuthority<AuthorityError> for Authority {
    fn validate_default_callable_reference_target(
        &mut self,
        _template: &ExportDefaultTemplateV1,
        _target: &ExportDefaultCallableTargetV1,
    ) -> Result<(), AuthorityError> {
        self.target(ExportDefaultReferenceKindV1::Callable)
    }

    fn validate_default_constructor_reference_target(
        &mut self,
        _template: &ExportDefaultTemplateV1,
        _target: &DefaultConstructorRefV1,
    ) -> Result<(), AuthorityError> {
        self.target(ExportDefaultReferenceKindV1::Constructor)
    }

    fn validate_default_type_reference_target(
        &mut self,
        _template: &ExportDefaultTemplateV1,
        _target: &SignatureTypeKey,
    ) -> Result<(), AuthorityError> {
        self.target(ExportDefaultReferenceKindV1::Type)
    }

    fn validate_default_global_reference_target(
        &mut self,
        _template: &ExportDefaultTemplateV1,
        _target: PersistentPropertyId,
    ) -> Result<(), AuthorityError> {
        self.target(ExportDefaultReferenceKindV1::Global)
    }

    fn validate_default_singleton_reference_target(
        &mut self,
        _template: &ExportDefaultTemplateV1,
        _target: PersistentObjectValueId,
    ) -> Result<(), AuthorityError> {
        self.target(ExportDefaultReferenceKindV1::Singleton)
    }

    fn validate_default_field_reference_target(
        &mut self,
        _template: &ExportDefaultTemplateV1,
        _target: &DefaultFieldRefV1,
    ) -> Result<(), AuthorityError> {
        self.target(ExportDefaultReferenceKindV1::Field)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum AuthorityError {
    MissingLocalFunction,
    Rejected(ExportDefaultReferenceKindV1),
    UnknownNominal,
}

impl std::fmt::Display for AuthorityError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for AuthorityError {}

impl crate::DefaultLocalFunctionSignatureAuthority<AuthorityError> for Authority {
    fn default_local_function_own_binder_arity(
        &mut self,
        declaration: scoop_identity::CallableTemplateOrigin,
    ) -> Result<u32, AuthorityError> {
        self.local_keys
            .get(&declaration)
            .map(|key| key.duplicate_signature().type_parameter_count())
            .ok_or(AuthorityError::MissingLocalFunction)
    }
}
