use scoop_identity::{
    CallableTemplateOrigin, CanonicalIdentifier, ConeIdentity, DeclarationScope, DefinitionOrigin,
    DefinitionOwnerAtom, DefinitionOwnerChain, Effect, GcEffect, LocalValueSelector, PackagePath,
    PersistentGenericFunctionId, PersistentGenericTypeId, PersistentTypeId, SignatureTypeKey,
    SourceContextKey, SourceDeclarationKey, SourceDeclarationSite, SourceIdentity,
    SourceNominalKind, SourceSpan, StructuralDefinitionPath, StructuralDefinitionSiteRole,
    StructuralPathSegment,
};

use super::*;
use crate::{
    CallableDeclarationIdentityShapeV1, CallableImplementationV1, CallableInfixV1,
    CallableModalityV1, CallableOperatorRoleV1, CallableParameterCallingV1, CallableSafetyV1,
    CallableSourceEffectsV1, CallableSourceParameterV1, CanonicalBinderListV1,
    CanonicalBinderUseListV1, CanonicalBooleanV1, CanonicalCallableSourceParametersV1,
    CanonicalSourceParameterShapesV1, CanonicalTemplateLocalTableV1,
    CanonicalTemplateValueParametersV1, DefaultExpressionKindV1, DefaultExpressionV1,
    DefaultTemplateProviderShapeV1, ExportDefaultBodyV1, ExportDefaultReferenceSetV1,
    ExportDefaultTemplateKeyV1, ExportDefinitionSourceV1, NominalInterfaceShapeAuthority,
    OptionalTemplateReceiverV1, PersistentLexicalRootV1, PublicDeclarationOwnerV1,
    PublicLookupAccessV1, PublicNominalShapeV1, SourceParameterShapeV1, TemplateLocalDefinitionV1,
    TemplateLocalRecordV1, TemplateValueParameterV1, TypeParameterBinderV1, TypeParameterBoundsV1,
};

#[test]
fn validates_an_inherited_two_frame_default_contract() {
    let fixture = Fixture::new();
    let template = fixture.template(
        fixture.provider_result(),
        fixture.identity_mapping(),
        CanonicalBooleanV1::False,
        CanonicalBooleanV1::False,
    );
    let callable = fixture.callable(Effect::Ordinary);
    let source = fixture.source();
    let mut authority = fixture.authority();

    assert_eq!(
        template.validate_contract_semantics(&callable, &source, &mut authority),
        Ok(())
    );
    assert_eq!(authority.inherited_validations, 1);
}

#[test]
fn validates_result_type_before_and_after_provider_substitution() {
    let fixture = Fixture::new();
    let callable = fixture.callable(Effect::Ordinary);
    let source = fixture.source();
    let invalid_result = SignatureTypeKey::Binder { depth: 2, index: 0 };
    let invalid = fixture.template(
        invalid_result,
        fixture.identity_mapping(),
        CanonicalBooleanV1::False,
        CanonicalBooleanV1::False,
    );

    assert_eq!(
        invalid.validate_contract_semantics(&callable, &source, &mut fixture.authority()),
        Err(
            ExportDefaultTemplateContractSemanticValidationError::ResultType(
                SignatureTypeSemanticError::BinderScope(
                    crate::SignatureBinderScopeError::DepthOutOfRange {
                        depth: 2,
                        available_depths: 2,
                    }
                )
            )
        )
    );

    let wrong_mapping =
        CanonicalBinderUseListV1::try_new(vec![binder(0, 1), binder(0, 1)]).unwrap();
    let mismatched = fixture.template(
        fixture.provider_result(),
        wrong_mapping,
        CanonicalBooleanV1::False,
        CanonicalBooleanV1::False,
    );
    assert_eq!(
        mismatched.validate_contract_semantics(&callable, &source, &mut fixture.authority()),
        Err(
            ExportDefaultTemplateContractSemanticValidationError::ResultMismatch {
                expected: Box::new(binder(0, 0)),
                actual: Box::new(binder(0, 1)),
            }
        )
    );
}

#[test]
fn suspend_permission_matches_the_publishing_callable() {
    let fixture = Fixture::new();
    let source = fixture.source();

    let ordinary = fixture.template(
        fixture.provider_result(),
        fixture.identity_mapping(),
        CanonicalBooleanV1::True,
        CanonicalBooleanV1::False,
    );
    assert_eq!(
        ordinary.validate_contract_semantics(
            &fixture.callable(Effect::Ordinary),
            &source,
            &mut fixture.authority(),
        ),
        Err(
            ExportDefaultTemplateContractSemanticValidationError::SuspendPermission {
                expected: CanonicalBooleanV1::False,
                actual: CanonicalBooleanV1::True,
            }
        )
    );

    let suspend = fixture.template(
        fixture.provider_result(),
        fixture.identity_mapping(),
        CanonicalBooleanV1::False,
        CanonicalBooleanV1::False,
    );
    assert_eq!(
        suspend.validate_contract_semantics(
            &fixture.callable(Effect::Suspend),
            &source,
            &mut fixture.authority(),
        ),
        Err(
            ExportDefaultTemplateContractSemanticValidationError::SuspendPermission {
                expected: CanonicalBooleanV1::True,
                actual: CanonicalBooleanV1::False,
            }
        )
    );
}

#[test]
fn routes_local_and_value_parameter_contract_failures() {
    let fixture = Fixture::new();
    let callable = fixture.callable(Effect::Ordinary);
    let source = fixture.source();
    let invalid_type = SignatureTypeKey::Binder { depth: 2, index: 0 };
    let invalid_local = fixture.template(
        fixture.provider_result(),
        fixture.identity_mapping(),
        CanonicalBooleanV1::False,
        CanonicalBooleanV1::False,
    );
    let selector = parameter_selector(0);
    let invalid_local = fixture.with_parameter_local(invalid_local, invalid_type, false);

    assert_eq!(
        invalid_local.validate_contract_semantics(&callable, &source, &mut fixture.authority()),
        Err(
            ExportDefaultTemplateContractSemanticValidationError::LocalTypes(
                TemplateLocalTypeSemanticValidationError {
                    index: 0,
                    selector: selector.clone(),
                    error: SignatureTypeSemanticError::BinderScope(
                        crate::SignatureBinderScopeError::DepthOutOfRange {
                            depth: 2,
                            available_depths: 2,
                        }
                    ),
                }
            )
        )
    );

    let mutable_local = fixture.template(
        fixture.provider_result(),
        fixture.identity_mapping(),
        CanonicalBooleanV1::False,
        CanonicalBooleanV1::True,
    );
    assert_eq!(
        mutable_local.validate_contract_semantics(&callable, &source, &mut fixture.authority()),
        Err(
            ExportDefaultTemplateContractSemanticValidationError::ValueParameters(
                TemplateValueParameterSemanticValidationError::MutableLocal { position: 0 }
            )
        )
    );
}

#[test]
fn rejects_mismatched_owner_interfaces_and_identity_shape() {
    let fixture = Fixture::new();
    let template = fixture.template(
        fixture.provider_result(),
        fixture.identity_mapping(),
        CanonicalBooleanV1::False,
        CanonicalBooleanV1::False,
    );
    let callable = fixture.callable(Effect::Ordinary);
    let source = fixture.source();

    let other = fixture.other_callable();
    assert_eq!(
        template.validate_contract_semantics(&other, &source, &mut fixture.authority()),
        Err(
            ExportDefaultTemplateContractSemanticValidationError::CallableOwner {
                expected: fixture.key.owner(),
                actual: other.declaration(),
            }
        )
    );

    let mut authority = fixture.authority();
    authority.identity = CallableDeclarationIdentityShapeV1::new(
        PublicDeclarationOwnerV1::TopLevel,
        1,
        0,
        None,
        vec![binder(0, 1), binder(0, 0)],
    );
    assert_eq!(
        template.validate_contract_semantics(&callable, &source, &mut authority),
        Err(
            ExportDefaultTemplateContractSemanticValidationError::CallableInterface(
                CallableInterfaceSemanticValidationError::TypeParameterArity {
                    expected: 1,
                    actual: 2,
                }
            )
        )
    );
}

struct Fixture {
    key: ExportDefaultTemplateKeyV1,
    provider: PersistentLexicalRootV1,
    definition_path: StructuralDefinitionPath,
    origin: ExportDefinitionSourceV1,
    identity: CallableDeclarationIdentityShapeV1,
}

impl Fixture {
    fn new() -> Self {
        let parameter_types = vec![binder(0, 1), binder(0, 0)];
        let owner_key = SourceDeclarationKey::function(
            top_level_site(),
            identifier("publishedDefault"),
            2,
            None,
            parameter_types.clone(),
        );
        let owner = CallableTemplateOrigin::GenericFunction(
            PersistentGenericFunctionId::from_source_declaration(&owner_key).unwrap(),
        );

        let nominal_key = SourceDeclarationKey::nominal(
            top_level_site(),
            identifier("ProviderOwner"),
            SourceNominalKind::Class,
            1,
        );
        let nominal = PersistentGenericTypeId::from_source_declaration(&nominal_key).unwrap();
        let provider_key = SourceDeclarationKey::function(
            owned_site(DefinitionOwnerAtom::GenericType(nominal)),
            identifier("providedDefault"),
            1,
            None,
            vec![binder(0, 0), binder(1, 0)],
        );
        let provider = PersistentLexicalRootV1::GenericFunction(
            PersistentGenericFunctionId::from_source_declaration(&provider_key).unwrap(),
        );
        let definition_path = StructuralDefinitionPath::from_first(
            StructuralPathSegment::new(StructuralDefinitionSiteRole::DefaultValue, 0),
            [],
        );
        Self {
            key: ExportDefaultTemplateKeyV1::new(owner, 1),
            provider,
            definition_path,
            origin: origin(),
            identity: CallableDeclarationIdentityShapeV1::new(
                PublicDeclarationOwnerV1::TopLevel,
                2,
                0,
                None,
                parameter_types,
            ),
        }
    }

    fn callable(&self, effect: Effect) -> CallableInterfaceRecordV1 {
        CallableInterfaceRecordV1::try_new(
            self.key.owner(),
            PublicDeclarationOwnerV1::TopLevel,
            binders(2),
            None,
            source_shapes(vec![binder(0, 1), binder(0, 0)]),
            binder(0, 0),
            effects(effect),
            CallableModalityV1::Final,
            PublicLookupAccessV1::DirectOnly,
        )
        .unwrap()
    }

    fn other_callable(&self) -> CallableInterfaceRecordV1 {
        let key = SourceDeclarationKey::function(
            top_level_site(),
            identifier("otherDefault"),
            2,
            None,
            vec![binder(0, 1), binder(0, 0)],
        );
        CallableInterfaceRecordV1::try_new(
            CallableTemplateOrigin::GenericFunction(
                PersistentGenericFunctionId::from_source_declaration(&key).unwrap(),
            ),
            PublicDeclarationOwnerV1::TopLevel,
            binders(2),
            None,
            source_shapes(vec![binder(0, 1), binder(0, 0)]),
            binder(0, 0),
            effects(Effect::Ordinary),
            CallableModalityV1::Final,
            PublicLookupAccessV1::DirectOnly,
        )
        .unwrap()
    }

    fn source(&self) -> CallableSourceInterfaceV1 {
        let parameters = vec![
            CallableSourceParameterV1::new(
                identifier("p0"),
                binder(0, 1),
                CallableParameterCallingV1::Required,
                self.origin.clone(),
            ),
            CallableSourceParameterV1::new(
                identifier("p1"),
                binder(0, 0),
                CallableParameterCallingV1::Default { template: self.key },
                self.origin.clone(),
            ),
        ];
        CallableSourceInterfaceV1::try_new(
            self.key.owner(),
            CanonicalCallableSourceParametersV1::try_new(parameters).unwrap(),
        )
        .unwrap()
    }

    fn template(
        &self,
        result: SignatureTypeKey,
        type_parameters: CanonicalBinderUseListV1,
        allows_suspend: CanonicalBooleanV1,
        parameter_mutable: CanonicalBooleanV1,
    ) -> ExportDefaultTemplateV1 {
        let locals = parameter_locals(
            SignatureTypeKey::Binder { depth: 0, index: 0 },
            parameter_mutable,
            self.origin.clone(),
        );
        let body = ExportDefaultBodyV1::try_new(
            Vec::new(),
            DefaultExpressionV1::try_new(
                DefaultExpressionKindV1::UnitLiteral,
                result.clone(),
                self.origin.clone(),
            )
            .unwrap(),
        )
        .unwrap();
        ExportDefaultTemplateV1::try_new(
            self.key,
            self.provider,
            self.definition_path.clone(),
            locals,
            body,
            result,
            allows_suspend,
            type_parameters,
            OptionalTemplateReceiverV1::Absent,
            CanonicalTemplateValueParametersV1::try_new(vec![
                TemplateValueParameterV1::try_new(0, parameter_selector(0)).unwrap(),
            ])
            .unwrap(),
            ExportDefaultReferenceSetV1::default(),
            self.origin.clone(),
        )
        .unwrap()
    }

    fn with_parameter_local(
        &self,
        template: ExportDefaultTemplateV1,
        value_type: SignatureTypeKey,
        mutable: bool,
    ) -> ExportDefaultTemplateV1 {
        let locals = parameter_locals(
            value_type,
            CanonicalBooleanV1::from(mutable),
            self.origin.clone(),
        );
        ExportDefaultTemplateV1::try_new(
            template.key(),
            template.definition_root(),
            template.definition_path().clone(),
            locals,
            template.body().clone(),
            template.result().clone(),
            template.allows_suspend(),
            template.type_parameters().clone(),
            template.receiver().clone(),
            template.value_parameters().clone(),
            template.references().clone(),
            template.definition_origin().clone(),
        )
        .unwrap()
    }

    fn provider_result(&self) -> SignatureTypeKey {
        SignatureTypeKey::Binder { depth: 1, index: 0 }
    }

    fn identity_mapping(&self) -> CanonicalBinderUseListV1 {
        CanonicalBinderUseListV1::try_new(vec![binder(0, 0), binder(0, 1)]).unwrap()
    }

    fn authority(&self) -> Authority {
        Authority {
            declaration: self.key.owner(),
            identity: self.identity.clone(),
            provider: self.provider,
            definition_path: self.definition_path.clone(),
            inherited_validations: 0,
        }
    }
}

struct Authority {
    declaration: CallableTemplateOrigin,
    identity: CallableDeclarationIdentityShapeV1,
    provider: PersistentLexicalRootV1,
    definition_path: StructuralDefinitionPath,
    inherited_validations: usize,
}

impl NominalInterfaceShapeAuthority<AuthorityError> for Authority {
    fn concrete_nominal_shape(
        &mut self,
        declaration: PersistentTypeId,
    ) -> Result<PublicNominalShapeV1, AuthorityError> {
        Err(AuthorityError::ConcreteNominal(declaration))
    }

    fn generic_nominal_shape(
        &mut self,
        declaration: PersistentGenericTypeId,
    ) -> Result<PublicNominalShapeV1, AuthorityError> {
        Err(AuthorityError::GenericNominal(declaration))
    }
}

impl CallableInterfaceSemanticAuthority<AuthorityError> for Authority {
    fn callable_declaration_identity_shape(
        &mut self,
        declaration: CallableTemplateOrigin,
    ) -> Result<CallableDeclarationIdentityShapeV1, AuthorityError> {
        if declaration == self.declaration {
            Ok(self.identity.clone())
        } else {
            Err(AuthorityError::Callable(declaration))
        }
    }
}

impl DefaultTemplateRootSemanticAuthority<AuthorityError> for Authority {
    fn default_template_provider_shape(
        &mut self,
        root: PersistentLexicalRootV1,
        path: &StructuralDefinitionPath,
    ) -> Result<DefaultTemplateProviderShapeV1, AuthorityError> {
        if root != self.provider || path != &self.definition_path {
            return Err(AuthorityError::Provider);
        }
        Ok(DefaultTemplateProviderShapeV1::try_new(1, 1).unwrap())
    }

    fn validate_inherited_default_provider(
        &mut self,
        _key: ExportDefaultTemplateKeyV1,
        root: PersistentLexicalRootV1,
        path: &StructuralDefinitionPath,
    ) -> Result<(), AuthorityError> {
        if root != self.provider || path != &self.definition_path {
            return Err(AuthorityError::Inherited);
        }
        self.inherited_validations += 1;
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum AuthorityError {
    Callable(CallableTemplateOrigin),
    ConcreteNominal(PersistentTypeId),
    GenericNominal(PersistentGenericTypeId),
    Provider,
    Inherited,
}

impl std::fmt::Display for AuthorityError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "missing default-contract authority: {self:?}")
    }
}

impl std::error::Error for AuthorityError {}

fn parameter_locals(
    value_type: SignatureTypeKey,
    mutable: CanonicalBooleanV1,
    origin: ExportDefinitionSourceV1,
) -> CanonicalTemplateLocalTableV1 {
    CanonicalTemplateLocalTableV1::try_new(vec![
        TemplateLocalRecordV1::try_new(
            parameter_selector(0),
            value_type,
            mutable,
            TemplateLocalDefinitionV1::Source(origin),
        )
        .unwrap(),
    ])
    .unwrap()
}

fn binders(count: u32) -> CanonicalBinderListV1 {
    CanonicalBinderListV1::try_new(
        (0..count)
            .map(|index| {
                TypeParameterBinderV1::new(
                    identifier(&format!("T{index}")),
                    TypeParameterBoundsV1::Unconstrained,
                )
            })
            .collect(),
    )
    .unwrap()
}

fn source_shapes(types: Vec<SignatureTypeKey>) -> CanonicalSourceParameterShapesV1 {
    CanonicalSourceParameterShapesV1::try_new(
        types
            .into_iter()
            .enumerate()
            .map(|(index, value_type)| {
                SourceParameterShapeV1::new(identifier(&format!("p{index}")), value_type)
            })
            .collect(),
    )
    .unwrap()
}

fn effects(execution: Effect) -> CallableSourceEffectsV1 {
    CallableSourceEffectsV1::try_new(
        execution,
        CallableSafetyV1::Safe,
        GcEffect::Managed,
        CallableImplementationV1::Scoop,
        CallableOperatorRoleV1::None,
        CallableInfixV1::Ordinary,
    )
    .unwrap()
}

fn top_level_site() -> SourceDeclarationSite {
    SourceDeclarationSite::new(
        ConeIdentity::SINGLE_FILE,
        PackagePath::root(),
        DefinitionOwnerChain::top_level(),
        DeclarationScope::ConeWide,
    )
    .unwrap()
}

fn owned_site(owner: DefinitionOwnerAtom) -> SourceDeclarationSite {
    SourceDeclarationSite::new(
        ConeIdentity::SINGLE_FILE,
        PackagePath::root(),
        DefinitionOwnerChain::from_outer_to_inner(vec![owner]),
        DeclarationScope::ConeWide,
    )
    .unwrap()
}

fn origin() -> ExportDefinitionSourceV1 {
    let source = SourceIdentity::single_file();
    let context = SourceContextKey::File {
        source: source.clone(),
    };
    ExportDefinitionSourceV1::new(
        DefinitionOrigin::new(source, SourceSpan::new(1, 2).unwrap(), &context).unwrap(),
    )
}

fn identifier(value: &str) -> CanonicalIdentifier {
    CanonicalIdentifier::new(value).unwrap()
}

const fn binder(depth: u32, index: u32) -> SignatureTypeKey {
    SignatureTypeKey::Binder { depth, index }
}

const fn parameter_selector(position: u32) -> LocalValueSelector {
    LocalValueSelector::Parameter {
        declaration_index: position,
    }
}
