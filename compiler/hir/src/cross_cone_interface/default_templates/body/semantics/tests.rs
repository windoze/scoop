use scoop_identity::{
    CallableTemplateOrigin, ConeIdentity, DefinitionOrigin, NormalizedSourcePath,
    PersistentGenericTypeId, PersistentTypeId, SignatureTypeKey, SourceContextKey, SourceIdentity,
    SourceSpan,
};
use scoop_wire::{BudgetMeter, DecodeLimits, ResourceKind, WireErrorKind, WirePath};

use super::*;
use crate::cross_cone_interface::default_templates::body::expression_test_support::{
    Fixture, definition_path,
};
use crate::{
    CanonicalBooleanV1, DefaultBinderRefV1, DefaultBoundCallableRefV1,
    DefaultBoundCallableSourceV1, DefaultCallableBodyTypeArgumentsV1, DefaultCaptureV1,
    DefaultExpressionKindV1, DefaultExpressionV1, DefaultLambdaV1, DefaultMethodCalleeV1,
    DefaultStatementKindV1, DefaultStatementV1, OptionalDefaultStatementListV1,
    PublicNominalShapeV1,
};

#[test]
fn validates_nested_body_types_and_origins_with_one_shared_meter() {
    let fixture = Fixture::new();
    let origin = fixture.origin();
    let nested = statement(
        DefaultStatementKindV1::Expr(Box::new(expression(
            DefaultExpressionKindV1::UnitLiteral,
            binder(0, 0),
            origin.clone(),
        ))),
        origin.clone(),
    );
    let branch = statement(
        DefaultStatementKindV1::If {
            condition: Box::new(expression(
                DefaultExpressionKindV1::BooleanLiteral(CanonicalBooleanV1::True),
                binder(0, 0),
                origin.clone(),
            )),
            then_body: vec![nested],
            else_body: OptionalDefaultStatementListV1::absent(),
        },
        origin.clone(),
    );
    let body = ExportDefaultBodyV1::try_new(
        vec![branch],
        expression(DefaultExpressionKindV1::UnitLiteral, binder(0, 0), origin),
    )
    .unwrap();
    let mut authority = Authority::accepting(ConeIdentity::CORE);
    let mut meter = BudgetMeter::new(DecodeLimits::default());

    assert_eq!(
        body.validate_provider_envelope_semantics(
            provider(),
            &mut authority,
            &mut meter,
            &WirePath::root().field(7),
        ),
        Ok(())
    );
    assert_eq!(authority.origin_validations, 5);
    assert!(meter.usage().decoded_nodes > 5);
    assert!(meter.usage().decoded_edges > 5);
    assert!(meter.usage().validation_work_units > meter.usage().decoded_nodes);
}

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

#[test]
fn checks_body_depth_before_scheduling_a_child() {
    let fixture = Fixture::new();
    let body = ExportDefaultBodyV1::try_new(
        Vec::new(),
        expression(
            DefaultExpressionKindV1::UnitLiteral,
            binder(0, 0),
            fixture.origin(),
        ),
    )
    .unwrap();
    let path = WirePath::root().field(7).index(0).field(5);
    let mut meter = BudgetMeter::new(DecodeLimits {
        semantic_recursion: 1,
        ..DecodeLimits::default()
    });
    let error = body
        .validate_provider_envelope_semantics(
            provider(),
            &mut Authority::accepting(ConeIdentity::CORE),
            &mut meter,
            &path,
        )
        .unwrap_err();

    assert!(matches!(
        error,
        DefaultBodyProviderEnvelopeSemanticValidationError::Resource(ref error)
            if error.kind()
                == &WireErrorKind::LimitExceeded {
                    resource: ResourceKind::SemanticRecursion,
                    limit: 1,
                    observed: 2,
                }
                && error.path() == &path
    ));
    assert_eq!(meter.usage().decoded_nodes, 1);
    assert_eq!(meter.usage().decoded_edges, 0);
    assert_eq!(meter.usage().validation_work_units, 1);
}

fn validate(
    body: &ExportDefaultBodyV1,
    authority: &mut Authority,
) -> Result<(), DefaultBodyProviderEnvelopeSemanticValidationError<AuthorityError>> {
    body.validate_provider_envelope_semantics(
        provider(),
        authority,
        &mut BudgetMeter::new(DecodeLimits::default()),
        &WirePath::root(),
    )
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
