use scoop_identity::{
    CallableTemplateOrigin, ConeIdentity, DefinitionOrigin, NormalizedSourcePath,
    PersistentGenericTypeId, PersistentTypeId, SignatureTypeKey, SourceContextKey, SourceIdentity,
    SourceSpan,
};
use scoop_wire::WirePath;

use super::*;
use crate::cross_cone_interface::default_templates::body::expression_test_support::{
    Fixture, definition_path,
};
use crate::{
    DefaultBinderRefV1, DefaultBoundCallableRefV1, DefaultBoundCallableSourceV1,
    DefaultCallableBodyTypeArgumentsV1, DefaultCaptureV1, DefaultExpressionKindV1,
    DefaultExpressionV1, DefaultLambdaV1, DefaultMethodCalleeV1, DefaultStatementKindV1,
    DefaultStatementV1, PublicNominalShapeV1,
};

#[test]
fn rejects_an_out_of_scope_type_inside_a_nested_capture() {
    let fixture = Fixture::new();
    let origin = fixture.origin();
    let capture = DefaultCaptureV1::new(fixture.local(), binder(1, 0), origin.clone());
    let lambda = DefaultLambdaV1::try_new(
        fixture.generated,
        definition_path(),
        binder(0, 0),
        DefaultCallableBodyTypeArgumentsV1::lexical(),
        vec![capture],
        0,
    )
    .unwrap();
    let body = ExportDefaultBodyV1::try_new(
        Vec::new(),
        expression(
            DefaultExpressionKindV1::Lambda(lambda),
            binder(0, 0),
            origin.clone(),
        ),
    )
    .unwrap();

    assert_eq!(
        validate(&body, &mut Authority::accepting(ConeIdentity::CORE)),
        Err(DefaultBodyProviderEnvelopeSemanticValidationError::Type {
            site: DefaultBodyProviderTypeSiteV1::CaptureValue,
            definition_origin: Box::new(origin),
            error: Box::new(SignatureTypeSemanticError::BinderScope(
                crate::SignatureBinderScopeError::DepthOutOfRange {
                    depth: 1,
                    available_depths: 1,
                }
            )),
        })
    );
}

#[test]
fn rejects_a_foreign_statement_origin_before_visiting_its_kind() {
    let fixture = Fixture::new();
    let foreign = origin(ConeIdentity::SINGLE_FILE, "main.scoop", 8);
    let body = ExportDefaultBodyV1::try_new(
        vec![statement(DefaultStatementKindV1::Break, foreign.clone())],
        expression(
            DefaultExpressionKindV1::UnitLiteral,
            binder(0, 0),
            fixture.origin(),
        ),
    )
    .unwrap();

    assert_eq!(
        validate(&body, &mut Authority::accepting(ConeIdentity::CORE)),
        Err(DefaultBodyProviderEnvelopeSemanticValidationError::Origin {
            site: DefaultBodyOriginSiteV1::Statement,
            definition_origin: Box::new(foreign),
            error: Box::new(ExportDefinitionSourceSemanticValidationError::Cone {
                expected: ConeIdentity::CORE,
                actual: ConeIdentity::SINGLE_FILE,
            }),
        })
    );
}

#[test]
fn validates_bound_receiver_binders_against_the_provider_scope() {
    let fixture = Fixture::new();
    let origin = fixture.origin();
    let bound = DefaultBoundCallableRefV1::new(
        DefaultBinderRefV1::new(1, 0),
        DefaultBoundCallableSourceV1::Interface {
            bound: binder(0, 0),
            member: CallableTemplateOrigin::Function(fixture.function),
        },
        binder(0, 0),
    );
    let receiver = expression(
        DefaultExpressionKindV1::UnitLiteral,
        binder(0, 0),
        origin.clone(),
    );
    let body = ExportDefaultBodyV1::try_new(
        Vec::new(),
        expression(
            DefaultExpressionKindV1::MethodCall {
                receiver: Box::new(receiver),
                callee: DefaultMethodCalleeV1::Bound(bound),
                arguments: Vec::new(),
            },
            binder(0, 0),
            origin.clone(),
        ),
    )
    .unwrap();

    assert_eq!(
        validate(&body, &mut Authority::accepting(ConeIdentity::CORE)),
        Err(DefaultBodyProviderEnvelopeSemanticValidationError::Binder {
            site: DefaultBodyProviderTypeSiteV1::BoundCallableReceiverParameter,
            definition_origin: Box::new(origin),
            error: crate::SignatureBinderScopeError::DepthOutOfRange {
                depth: 1,
                available_depths: 1,
            },
        })
    );
}

fn validate(
    body: &ExportDefaultBodyV1,
    authority: &mut Authority,
) -> Result<(), DefaultBodyProviderEnvelopeSemanticValidationError<AuthorityError>> {
    body.validate_provider_envelope_semantics(provider(), authority, &WirePath::root())
}

fn provider() -> DefaultTemplateProviderShapeV1 {
    DefaultTemplateProviderShapeV1::try_new(0, 1).unwrap()
}

fn expression(
    kind: DefaultExpressionKindV1,
    result_type: SignatureTypeKey,
    origin: ExportDefinitionSourceV1,
) -> DefaultExpressionV1 {
    DefaultExpressionV1::try_new(kind, result_type, origin).unwrap()
}

fn statement(kind: DefaultStatementKindV1, origin: ExportDefinitionSourceV1) -> DefaultStatementV1 {
    DefaultStatementV1::try_new(kind, origin).unwrap()
}

const fn binder(depth: u32, index: u32) -> SignatureTypeKey {
    SignatureTypeKey::Binder { depth, index }
}

fn origin(cone: ConeIdentity, path: &str, point: u64) -> ExportDefinitionSourceV1 {
    let source = SourceIdentity::new(cone, NormalizedSourcePath::new(path).unwrap()).unwrap();
    let context = SourceContextKey::File {
        source: source.clone(),
    };
    ExportDefinitionSourceV1::new(
        DefinitionOrigin::new(source, SourceSpan::new(point, point + 1).unwrap(), &context)
            .unwrap(),
    )
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum AuthorityError {
    MissingLocalFunction,
    MissingNominal,
}

impl std::fmt::Display for AuthorityError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for AuthorityError {}

struct Authority {
    current: ConeIdentity,
    origin_validations: usize,
}

impl Authority {
    const fn accepting(current: ConeIdentity) -> Self {
        Self {
            current,
            origin_validations: 0,
        }
    }
}

impl ExportDefinitionSourceSemanticAuthority<AuthorityError> for Authority {
    fn current_cone(&self) -> ConeIdentity {
        self.current
    }

    fn validate_export_definition_source(
        &mut self,
        _: &ExportDefinitionSourceV1,
    ) -> Result<(), AuthorityError> {
        self.origin_validations += 1;
        Ok(())
    }
}

impl NominalInterfaceShapeAuthority<AuthorityError> for Authority {
    fn concrete_nominal_shape(
        &mut self,
        _: PersistentTypeId,
    ) -> Result<PublicNominalShapeV1, AuthorityError> {
        Err(AuthorityError::MissingNominal)
    }

    fn generic_nominal_shape(
        &mut self,
        _: PersistentGenericTypeId,
    ) -> Result<PublicNominalShapeV1, AuthorityError> {
        Err(AuthorityError::MissingNominal)
    }
}

impl crate::DefaultLocalFunctionSignatureAuthority<AuthorityError> for Authority {
    fn default_local_function_own_binder_arity(
        &mut self,
        _declaration: scoop_identity::CallableTemplateOrigin,
    ) -> Result<u32, AuthorityError> {
        Err(AuthorityError::MissingLocalFunction)
    }
}
