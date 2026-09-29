use scoop_identity::{
    CallableTemplateOrigin, PersistentGenericTypeId, PersistentTypeId, SignatureTypeKey,
};
use scoop_wire::WirePath;

use super::*;
use crate::cross_cone_interface::default_templates::body::expression_test_support::{
    Fixture, definition_path,
};
use crate::{
    DefaultBoundCallableRefV1, DefaultBoundCallableSourceV1, DefaultCallableBodyTypeArgumentsV1,
    DefaultCaptureV1, DefaultExpressionKindV1, DefaultExpressionV1, DefaultLambdaV1,
    DefaultMethodCalleeV1, PublicNominalShapeV1,
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
        validate(&body, &mut Authority),
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
fn validates_bound_receiver_binders_against_the_provider_scope() {
    let fixture = Fixture::new();
    let origin = fixture.origin();
    let bound = DefaultBoundCallableRefV1::new(
        SignatureTypeKey::Binder { depth: 1, index: 0 },
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
        validate(&body, &mut Authority),
        Err(DefaultBodyProviderEnvelopeSemanticValidationError::Type {
            site: DefaultBodyProviderTypeSiteV1::BoundCallableReceiverType,
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

fn validate(
    body: &ExportDefaultBodyV1,
    authority: &mut Authority,
) -> Result<(), DefaultBodyProviderEnvelopeSemanticValidationError<AuthorityError>> {
    body.validate_provider_types_semantics(provider(), authority, &WirePath::root())
}

fn provider() -> DefaultTemplateProviderShapeV1 {
    DefaultTemplateProviderShapeV1::try_new(0, 1).unwrap()
}

fn expression(
    kind: DefaultExpressionKindV1,
    result_type: SignatureTypeKey,
    origin: ExportDefinitionSourceV1,
) -> DefaultExpressionV1 {
    DefaultExpressionV1::try_new(
        kind,
        result_type,
        origin.clone(),
        scoop_identity::EvaluationOrigin::at_definition(origin.origin()),
    )
    .unwrap()
}

const fn binder(depth: u32, index: u32) -> SignatureTypeKey {
    SignatureTypeKey::Binder { depth, index }
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

struct Authority;

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
