use scoop_identity::{
    CallableTemplateOrigin, CanonicalIdentifier, ConeIdentity, DeclarationScope, DefinitionOrigin,
    DefinitionOwnerChain, Effect, GcEffect, PackagePath, PersistentFunctionId,
    PersistentGenericTypeId, PersistentTypeId, SignatureTypeKey, SourceContextKey,
    SourceDeclarationKey, SourceDeclarationSite, SourceIdentity, SourceNominalKind, SourceSpan,
    StructuralDefinitionPath, StructuralDefinitionSiteRole, StructuralPathSegment,
};

use super::*;
use crate::{
    CallableDeclarationIdentityShapeV1, CallableImplementationV1, CallableInfixV1,
    CallableInterfaceRecordV1, CallableInterfaceSemanticAuthority, CallableModalityV1,
    CallableOperatorRoleV1, CallableParameterCallingV1, CallableSafetyV1, CallableSourceEffectsV1,
    CallableSourceParameterV1, CanonicalBinderListV1, CanonicalBinderUseListV1, CanonicalBooleanV1,
    CanonicalCallableInterfacesV1, CanonicalCallableSourceInterfacesV1,
    CanonicalCallableSourceParametersV1, CanonicalSourceParameterShapesV1,
    CanonicalTemplateLocalTableV1, CanonicalTemplateValueParametersV1, DefaultExpressionKindV1,
    DefaultExpressionV1, DefaultTemplateOriginSemanticAuthority, DefaultTemplateProviderShapeV1,
    DefaultTemplateRootSemanticAuthority, ExportDefaultBodyV1, ExportDefaultReferenceSetV1,
    ExportDefaultTemplateKeyV1, ExportDefaultTemplateOriginSemanticValidationError,
    ExportDefaultTemplateV1, ExportDefinitionSourceSemanticAuthority, ExportDefinitionSourceV1,
    NominalInterfaceShapeAuthority, OptionalTemplateReceiverV1, PersistentLexicalRootV1,
    PublicDeclarationOwnerV1, PublicLookupAccessV1, PublicNominalKindV1, PublicNominalShapeV1,
    SourceParameterShapeV1,
};

#[test]
fn validates_every_envelope_and_the_exact_source_closure() {
    let fixture = Fixture::new();
    let templates = fixture.templates(CanonicalBooleanV1::False);
    let callables = fixture.callables(Effect::Ordinary);
    let sources = fixture.sources(true);
    let mut authority = fixture.authority();

    assert_eq!(
        templates.validate_envelope_semantics(&callables, &sources, &mut authority),
        Ok(())
    );
    assert_eq!(authority.definition_source_validations, 1);
    assert_eq!(authority.template_origin_validations, 1);
}

#[test]
fn requires_matching_callable_and_source_interfaces() {
    let fixture = Fixture::new();
    let templates = fixture.templates(CanonicalBooleanV1::False);
    let empty_callables = CanonicalCallableInterfacesV1::try_new(Vec::new()).unwrap();
    assert_eq!(
        templates.validate_envelope_semantics(
            &empty_callables,
            &fixture.sources(true),
            &mut fixture.authority(),
        ),
        Err(
            ExportDefaultTemplateSetEnvelopeSemanticValidationError::MissingCallable {
                index: 0,
                owner: fixture.owner,
            }
        )
    );

    let empty_sources = CanonicalCallableSourceInterfacesV1::try_new(Vec::new()).unwrap();
    assert_eq!(
        templates.validate_envelope_semantics(
            &fixture.callables(Effect::Ordinary),
            &empty_sources,
            &mut fixture.authority(),
        ),
        Err(
            ExportDefaultTemplateSetEnvelopeSemanticValidationError::MissingSource {
                index: 0,
                owner: fixture.owner,
            }
        )
    );
}

#[test]
fn routes_record_contract_and_origin_failures_with_table_identity() {
    let fixture = Fixture::new();
    let templates = fixture.templates(CanonicalBooleanV1::False);
    let callables = fixture.callables(Effect::Suspend);
    let sources = fixture.sources(true);

    assert_eq!(
        templates.validate_envelope_semantics(&callables, &sources, &mut fixture.authority()),
        Err(
            ExportDefaultTemplateSetEnvelopeSemanticValidationError::Contract {
                index: 0,
                key: fixture.key,
                error: Box::new(
                    ExportDefaultTemplateContractSemanticValidationError::SuspendPermission {
                        expected: CanonicalBooleanV1::True,
                        actual: CanonicalBooleanV1::False,
                    }
                ),
            }
        )
    );

    let mut authority = fixture.authority();
    authority.reject_template_origin = true;
    assert_eq!(
        templates.validate_envelope_semantics(
            &fixture.callables(Effect::Ordinary),
            &sources,
            &mut authority,
        ),
        Err(
            ExportDefaultTemplateSetEnvelopeSemanticValidationError::Origin {
                index: 0,
                key: fixture.key,
                error: Box::new(
                    ExportDefaultTemplateOriginSemanticValidationError::DefinitionRelation(
                        AuthorityError::TemplateOrigin
                    )
                ),
            }
        )
    );
}

#[test]
fn rejects_source_defaults_missing_from_the_template_table() {
    let fixture = Fixture::new();
    let templates = CanonicalExportDefaultTemplatesV1::try_new(Vec::new()).unwrap();

    assert_eq!(
        templates.validate_envelope_semantics(
            &fixture.callables(Effect::Ordinary),
            &fixture.sources(true),
            &mut fixture.authority(),
        ),
        Err(
            ExportDefaultTemplateSetEnvelopeSemanticValidationError::SourceClosure(
                ExportDefaultTemplateSourceClosureValidationError::MissingTemplate {
                    source_index: 0,
                    owner: fixture.owner,
                    parameter_position: 0,
                    key: fixture.key,
                }
            )
        )
    );
}

struct Fixture {
    function: PersistentFunctionId,
    owner: CallableTemplateOrigin,
    key: ExportDefaultTemplateKeyV1,
    value_type_id: PersistentTypeId,
    value_type: SignatureTypeKey,
    definition_path: StructuralDefinitionPath,
    origin: ExportDefinitionSourceV1,
}

impl Fixture {
    fn new() -> Self {
        let nominal_key = SourceDeclarationKey::nominal(
            top_level_site(),
            identifier("Value"),
            SourceNominalKind::Struct,
            0,
        );
        let value_type_id = PersistentTypeId::from_source_declaration(&nominal_key).unwrap();
        let value_type = SignatureTypeKey::Nominal(value_type_id);
        let function_key = SourceDeclarationKey::function(
            top_level_site(),
            identifier("withDefault"),
            0,
            None,
            vec![value_type.clone()],
        );
        let function = PersistentFunctionId::from_source_declaration(&function_key).unwrap();
        let owner = CallableTemplateOrigin::Function(function);
        Self {
            function,
            owner,
            key: ExportDefaultTemplateKeyV1::new(owner, 0),
            value_type_id,
            value_type,
            definition_path: StructuralDefinitionPath::from_first(
                StructuralPathSegment::new(StructuralDefinitionSiteRole::DefaultValue, 0),
                [],
            ),
            origin: origin(),
        }
    }

    fn callables(&self, effect: Effect) -> CanonicalCallableInterfacesV1 {
        let callable = CallableInterfaceRecordV1::try_new(
            self.owner,
            PublicDeclarationOwnerV1::TopLevel,
            CanonicalBinderListV1::try_new(Vec::new()).unwrap(),
            None,
            CanonicalSourceParameterShapesV1::try_new(vec![SourceParameterShapeV1::new(
                identifier("value"),
                self.value_type.clone(),
            )])
            .unwrap(),
            self.value_type.clone(),
            effects(effect),
            CallableModalityV1::Final,
            PublicLookupAccessV1::DirectOnly,
        )
        .unwrap();
        CanonicalCallableInterfacesV1::try_new(vec![callable]).unwrap()
    }

    fn sources(&self, has_default: bool) -> CanonicalCallableSourceInterfacesV1 {
        let calling = if has_default {
            CallableParameterCallingV1::Default { template: self.key }
        } else {
            CallableParameterCallingV1::Required
        };
        let source = CallableSourceInterfaceV1::try_new(
            self.owner,
            CanonicalCallableSourceParametersV1::try_new(vec![CallableSourceParameterV1::new(
                identifier("value"),
                self.value_type.clone(),
                calling,
                self.origin.clone(),
            )])
            .unwrap(),
        )
        .unwrap();
        CanonicalCallableSourceInterfacesV1::try_new(vec![source]).unwrap()
    }

    fn templates(&self, allows_suspend: CanonicalBooleanV1) -> CanonicalExportDefaultTemplatesV1 {
        let body = ExportDefaultBodyV1::try_new(
            Vec::new(),
            DefaultExpressionV1::try_new(
                DefaultExpressionKindV1::UnitLiteral,
                self.value_type.clone(),
                self.origin.clone(),
            )
            .unwrap(),
        )
        .unwrap();
        let template = ExportDefaultTemplateV1::try_new(
            self.key,
            PersistentLexicalRootV1::Function(self.function),
            self.definition_path.clone(),
            CanonicalTemplateLocalTableV1::try_new(Vec::new()).unwrap(),
            body,
            self.value_type.clone(),
            allows_suspend,
            CanonicalBinderUseListV1::try_new(Vec::new()).unwrap(),
            OptionalTemplateReceiverV1::Absent,
            CanonicalTemplateValueParametersV1::try_new(Vec::new()).unwrap(),
            ExportDefaultReferenceSetV1::default(),
            self.origin.clone(),
        )
        .unwrap();
        CanonicalExportDefaultTemplatesV1::try_new(vec![template]).unwrap()
    }

    fn authority(&self) -> Authority {
        Authority {
            owner: self.owner,
            value_type: self.value_type_id,
            definition_path: self.definition_path.clone(),
            definition_source_validations: 0,
            template_origin_validations: 0,
            reject_template_origin: false,
        }
    }
}

struct Authority {
    owner: CallableTemplateOrigin,
    value_type: PersistentTypeId,
    definition_path: StructuralDefinitionPath,
    definition_source_validations: usize,
    template_origin_validations: usize,
    reject_template_origin: bool,
}

impl NominalInterfaceShapeAuthority<AuthorityError> for Authority {
    fn concrete_nominal_shape(
        &mut self,
        declaration: PersistentTypeId,
    ) -> Result<PublicNominalShapeV1, AuthorityError> {
        if declaration == self.value_type {
            Ok(PublicNominalShapeV1::new(PublicNominalKindV1::Struct, 0))
        } else {
            Err(AuthorityError::ConcreteNominal(declaration))
        }
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
        if declaration != self.owner {
            return Err(AuthorityError::Callable(declaration));
        }
        Ok(CallableDeclarationIdentityShapeV1::new(
            PublicDeclarationOwnerV1::TopLevel,
            0,
            0,
            None,
            vec![SignatureTypeKey::Nominal(self.value_type)],
        ))
    }
}

impl DefaultTemplateRootSemanticAuthority<AuthorityError> for Authority {
    fn default_template_provider_shape(
        &mut self,
        root: PersistentLexicalRootV1,
        path: &StructuralDefinitionPath,
    ) -> Result<DefaultTemplateProviderShapeV1, AuthorityError> {
        if root.declaration() != self.owner || path != &self.definition_path {
            return Err(AuthorityError::Provider);
        }
        Ok(DefaultTemplateProviderShapeV1::try_new(0, 0).unwrap())
    }

    fn validate_inherited_default_provider(
        &mut self,
        _key: ExportDefaultTemplateKeyV1,
        _root: PersistentLexicalRootV1,
        _path: &StructuralDefinitionPath,
    ) -> Result<(), AuthorityError> {
        Err(AuthorityError::UnexpectedInheritedProvider)
    }
}

impl ExportDefinitionSourceSemanticAuthority<AuthorityError> for Authority {
    fn current_cone(&self) -> ConeIdentity {
        ConeIdentity::SINGLE_FILE
    }

    fn validate_export_definition_source(
        &mut self,
        _source: &ExportDefinitionSourceV1,
    ) -> Result<(), AuthorityError> {
        self.definition_source_validations += 1;
        Ok(())
    }
}

impl DefaultTemplateOriginSemanticAuthority<AuthorityError> for Authority {
    fn validate_default_template_origin(
        &mut self,
        key: ExportDefaultTemplateKeyV1,
        root: PersistentLexicalRootV1,
        path: &StructuralDefinitionPath,
        _origin: &ExportDefinitionSourceV1,
    ) -> Result<(), AuthorityError> {
        if self.reject_template_origin
            || key.owner() != self.owner
            || root.declaration() != self.owner
            || path != &self.definition_path
        {
            return Err(AuthorityError::TemplateOrigin);
        }
        self.template_origin_validations += 1;
        Ok(())
    }

    fn validate_default_template_local_origin(
        &mut self,
        _key: ExportDefaultTemplateKeyV1,
        _root: PersistentLexicalRootV1,
        _path: &StructuralDefinitionPath,
        _selector: &scoop_identity::LocalValueSelector,
        _origin: &ExportDefinitionSourceV1,
    ) -> Result<(), AuthorityError> {
        Err(AuthorityError::UnexpectedLocalOrigin)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum AuthorityError {
    Callable(CallableTemplateOrigin),
    ConcreteNominal(PersistentTypeId),
    GenericNominal(PersistentGenericTypeId),
    Provider,
    UnexpectedInheritedProvider,
    TemplateOrigin,
    UnexpectedLocalOrigin,
}

impl std::fmt::Display for AuthorityError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "missing table-envelope authority: {self:?}")
    }
}

impl std::error::Error for AuthorityError {}

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
