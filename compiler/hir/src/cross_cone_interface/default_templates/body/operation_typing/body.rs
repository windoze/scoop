use super::{
    DefaultBodyOperationAuthority, DefaultBodyValidationInputV1, authority::PublicAuthority,
};

use std::fmt;

use scoop_identity::{Effect, SignatureTypeKey};
use scoop_wire::{WireError, WirePath};

use crate::{
    CanonicalBooleanV1, DefaultAssignTargetV1, DefaultBindingActionV1, DefaultBindingPlanV1,
    DefaultBindingShapeV1, DefaultBindingTemporaryV1, DefaultCoreApplicationV1,
    DefaultExpressionV1, DefaultForIterationPlanV1, DefaultOperationCoreTypeV1,
    DefaultOperationEntityShapeKindV1, DefaultOperationEntityShapeV1, DefaultOperationEntityV1,
    DefaultOperationExpectedTypeShapeV1, DefaultOperationIntrinsicV1,
    DefaultOperationTypeRelationV1, DefaultOperationTypingSemanticAuthority,
    DefaultOperationValueRoleV1, DefaultPatternV1, DefaultStatementV1, DefaultTryV1,
    DefaultWhenArmV1, DefaultWhenFallbackV1, DefaultWhenGuardV1, DefaultWhenV1,
    ExportDefaultBodyV1, ExportDefaultOperationTypingValidationError, ExportDefaultTemplateV1,
    TemplateLocalRecordV1,
};

use super::Validator as ExpressionValidator;

mod binding;
mod pattern;
mod statement;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DefaultStatementOperationV1 {
    Return,
    ValueDeclaration,
    IfCondition,
    WhileCondition,
    WhenSubject,
    WhenPattern { arm: usize },
    WhenGuard { arm: usize },
    WhenFallback,
    Catch { index: usize },
    Throw,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DefaultAssignmentOperationV1 {
    Local,
    Global,
    Index,
    Field,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DefaultPatternOperationV1 {
    Binding,
    Literal,
    Variant,
    Tuple,
    Struct,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DefaultForOperationV1 {
    Source,
    IteratorCall,
    Conformance,
    Next,
    BindingSubject,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DefaultBindingActionOperationV1 {
    Project,
    Component,
    Bind,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DefaultBindingShapeOperationV1 {
    Binding,
    Tuple,
    Struct,
    Class,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DefaultBodyOperationV1 {
    Result,
    Statement(DefaultStatementOperationV1),
    Assignment(DefaultAssignmentOperationV1),
    Pattern(DefaultPatternOperationV1),
    For(DefaultForOperationV1),
    BindingAction {
        index: usize,
        kind: DefaultBindingActionOperationV1,
    },
    BindingShape(DefaultBindingShapeOperationV1),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DefaultBodyOperationTypingSiteV1 {
    operation: DefaultBodyOperationV1,
    role: DefaultOperationValueRoleV1,
}

impl DefaultBodyOperationTypingSiteV1 {
    pub const fn operation(self) -> DefaultBodyOperationV1 {
        self.operation
    }

    pub const fn role(self) -> DefaultOperationValueRoleV1 {
        self.role
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DefaultBodyOperationTypingProblemV1 {
    MissingReturnValue,
    MissingLocal,
    MutableBinding,
    ImmutableAssignmentTarget,
    InvalidAssignmentTarget,
    IndexOutOfBounds {
        index: u32,
        length: usize,
    },
    MissingBindingAction,
    MultipleBindingActions,
    NonCanonicalFieldIndex {
        expected: u32,
        actual: u32,
    },
    MissingCallableReceiver,
    UnexpectedCallableReceiver,
    UnexpectedCallableCaptures,
    NonOrdinaryProtocolCallable,
    FieldKindMismatch {
        expected: super::DefaultFieldOperationKindV1,
        actual: super::DefaultFieldOperationKindV1,
    },
}

#[derive(Debug, Eq, PartialEq)]
pub enum ExportDefaultBodyOperationTypingValidationError<E> {
    Expression(Box<ExportDefaultOperationTypingValidationError<E>>),
    Authority {
        site: DefaultBodyOperationTypingSiteV1,
        error: E,
    },
    Type {
        site: DefaultBodyOperationTypingSiteV1,
        expected: Box<SignatureTypeKey>,
        actual: Box<SignatureTypeKey>,
    },
    Arity {
        site: DefaultBodyOperationTypingSiteV1,
        expected: usize,
        actual: usize,
    },
    TypeShape {
        site: DefaultBodyOperationTypingSiteV1,
        expected: DefaultOperationExpectedTypeShapeV1,
        actual: Box<SignatureTypeKey>,
    },
    CoreApplication {
        site: DefaultBodyOperationTypingSiteV1,
        expected: DefaultOperationExpectedTypeShapeV1,
        actual: Option<DefaultOperationExpectedTypeShapeV1>,
    },
    EntityShape {
        site: DefaultBodyOperationTypingSiteV1,
        expected: DefaultOperationEntityShapeKindV1,
        actual: DefaultOperationEntityShapeKindV1,
    },
    Relation {
        site: DefaultBodyOperationTypingSiteV1,
        relation: DefaultOperationTypeRelationV1,
        source: Box<SignatureTypeKey>,
        target: Box<SignatureTypeKey>,
    },
    Mutability {
        site: DefaultBodyOperationTypingSiteV1,
        expected: CanonicalBooleanV1,
        actual: CanonicalBooleanV1,
    },
    Problem {
        site: DefaultBodyOperationTypingSiteV1,
        problem: DefaultBodyOperationTypingProblemV1,
    },
    Resource(WireError),
}

impl<E: fmt::Display> fmt::Display for ExportDefaultBodyOperationTypingValidationError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Expression(error) => write!(formatter, "invalid default expression: {error}"),
            Self::Authority { site, error } => {
                write!(
                    formatter,
                    "default body authority failed at {site:?}: {error}"
                )
            }
            Self::Type {
                site,
                expected,
                actual,
            } => write!(
                formatter,
                "default body type mismatch at {site:?}: expected {expected:?}, found {actual:?}"
            ),
            Self::Arity {
                site,
                expected,
                actual,
            } => write!(
                formatter,
                "default body arity mismatch at {site:?}: expected {expected}, found {actual}"
            ),
            Self::TypeShape {
                site,
                expected,
                actual,
            } => write!(
                formatter,
                "default body type at {site:?} must be {expected:?}, found {actual:?}"
            ),
            Self::CoreApplication {
                site,
                expected,
                actual,
            } => write!(
                formatter,
                "default body core application at {site:?} must be {expected:?}, found {actual:?}"
            ),
            Self::EntityShape {
                site,
                expected,
                actual,
            } => write!(
                formatter,
                "default body authority shape at {site:?} must be {expected:?}, found {actual:?}"
            ),
            Self::Relation {
                site,
                relation,
                source,
                target,
            } => write!(
                formatter,
                "default body relation {relation:?} does not hold at {site:?}: {source:?} -> {target:?}"
            ),
            Self::Mutability {
                site,
                expected,
                actual,
            } => write!(
                formatter,
                "default body mutability mismatch at {site:?}: expected {expected:?}, found {actual:?}"
            ),
            Self::Problem { site, problem } => {
                write!(
                    formatter,
                    "invalid default body operation at {site:?}: {problem:?}"
                )
            }
            Self::Resource(error) => {
                write!(
                    formatter,
                    "default body operation typing resource failure: {error}"
                )
            }
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error
    for ExportDefaultBodyOperationTypingValidationError<E>
{
}

impl ExportDefaultBodyV1 {
    /// Validates all statement, expression, pattern, assignment, and
    /// iteration operation types in this body against one provider authority.
    pub fn validate_operation_typing_semantics<A, E>(
        &self,
        template: &ExportDefaultTemplateV1,
        authority: &mut A,

        path: &WirePath,
    ) -> Result<(), ExportDefaultBodyOperationTypingValidationError<E>>
    where
        A: DefaultOperationTypingSemanticAuthority<E>,
    {
        DefaultBodyValidationInputV1::from(template).validate_operation_typing(
            self,
            &mut PublicAuthority {
                template,
                authority,
            },
            path,
        )
    }
}

impl DefaultBodyValidationInputV1<'_> {
    pub(crate) fn validate_operation_typing<A: DefaultBodyOperationAuthority<E>, E>(
        self,
        body: &ExportDefaultBodyV1,
        authority: &mut A,

        path: &WirePath,
    ) -> Result<(), ExportDefaultBodyOperationTypingValidationError<E>> {
        BodyValidator {
            template: self,
            authority,

            path,
            error: std::marker::PhantomData,
        }
        .run(body)
    }
}

struct BodyValidator<'a, A, E> {
    template: DefaultBodyValidationInputV1<'a>,
    authority: &'a mut A,

    path: &'a WirePath,
    error: std::marker::PhantomData<fn() -> E>,
}

impl<A, E> BodyValidator<'_, A, E>
where
    A: DefaultBodyOperationAuthority<E>,
{
    fn run(
        &mut self,
        body: &ExportDefaultBodyV1,
    ) -> Result<(), ExportDefaultBodyOperationTypingValidationError<E>> {
        let mut pending = Vec::new();
        scoop_wire::allocation::try_reserve(&mut pending, 1, self.path)
            .map_err(ExportDefaultBodyOperationTypingValidationError::Resource)?;
        pending.push(BodyNode::Body(body));
        while let Some(work) = pending.pop() {
            if let BodyNode::Expression(expression) = work {
                self.validate_expression(expression)?;
                continue;
            }

            match work {
                BodyNode::Body(body) => self.process_body(body, &mut pending)?,
                BodyNode::Statement(statement) => {
                    self.process_statement(statement, &mut pending)?;
                }
                BodyNode::Assignment { target, value } => {
                    self.process_assignment(target, value, &mut pending)?;
                }
                BodyNode::Pattern { pattern, subject } => {
                    self.process_pattern(pattern, &subject, &mut pending)?;
                }
                BodyNode::When(value) => self.process_when(value, &mut pending)?,
                BodyNode::WhenArm {
                    arm,
                    index,
                    subject,
                } => self.process_when_arm(arm, index, &subject, &mut pending)?,
                BodyNode::WhenGuard { guard, arm } => {
                    self.process_when_guard(guard, arm, &mut pending)?;
                }
                BodyNode::WhenFallback { fallback, subject } => {
                    self.process_when_fallback(fallback, &subject, &mut pending)?;
                }
                BodyNode::Try(value) => self.process_try(value, &mut pending)?,
                BodyNode::Catch { catch, index } => {
                    self.process_catch(catch, index, &mut pending)?;
                }
                BodyNode::For(plan) => self.process_for(plan, &mut pending)?,
                BodyNode::BindingPlan(plan) => {
                    self.process_binding_plan(plan, &mut pending)?;
                }
                BodyNode::BindingAction {
                    action,
                    index,
                    actions,
                } => self.process_binding_action(action, index, actions, &mut pending)?,
                BodyNode::BindingShape {
                    shape,
                    source,
                    actions,
                } => self.process_binding_shape(shape, source, actions, &mut pending)?,
                BodyNode::Expression(_) => unreachable!("expressions are dispatched separately"),
            }
        }
        Ok(())
    }

    fn process_body<'body>(
        &mut self,
        body: &'body ExportDefaultBodyV1,
        pending: &mut Vec<BodyNode<'body>>,
    ) -> Result<(), ExportDefaultBodyOperationTypingValidationError<E>> {
        self.expect_type(
            body.value().result_type(),
            self.template.result(),
            Self::site(
                DefaultBodyOperationV1::Result,
                DefaultOperationValueRoleV1::Result,
            ),
        )?;
        self.push_node(pending, BodyNode::Expression(body.value()))?;
        self.push_statements(pending, body.statements())
    }

    fn validate_expression(
        &mut self,
        expression: &DefaultExpressionV1,
    ) -> Result<(), ExportDefaultBodyOperationTypingValidationError<E>> {
        ExpressionValidator {
            template: self.template,
            authority: self.authority,

            path: self.path,
            error: std::marker::PhantomData,
        }
        .run_expression_at(expression)
        .map_err(|error| {
            ExportDefaultBodyOperationTypingValidationError::Expression(Box::new(error))
        })
    }

    const fn site(
        operation: DefaultBodyOperationV1,
        role: DefaultOperationValueRoleV1,
    ) -> DefaultBodyOperationTypingSiteV1 {
        DefaultBodyOperationTypingSiteV1 { operation, role }
    }

    fn push_node<'body>(
        &mut self,
        pending: &mut Vec<BodyNode<'body>>,
        node: BodyNode<'body>,
    ) -> Result<(), ExportDefaultBodyOperationTypingValidationError<E>> {
        scoop_wire::allocation::try_reserve(pending, 1, self.path)
            .map_err(ExportDefaultBodyOperationTypingValidationError::Resource)?;
        pending.push(node);
        Ok(())
    }

    fn push_statements<'body>(
        &mut self,
        pending: &mut Vec<BodyNode<'body>>,
        statements: &'body [DefaultStatementV1],
    ) -> Result<(), ExportDefaultBodyOperationTypingValidationError<E>> {
        for statement in statements.iter().rev() {
            self.push_node(pending, BodyNode::Statement(statement))?;
        }
        Ok(())
    }

    fn core_type(
        &mut self,
        role: DefaultOperationCoreTypeV1,
        site: DefaultBodyOperationTypingSiteV1,
    ) -> Result<SignatureTypeKey, ExportDefaultBodyOperationTypingValidationError<E>> {
        self.authority
            .canonical_default_operation_type(role, self.path)
            .map_err(
                |error| ExportDefaultBodyOperationTypingValidationError::Authority { site, error },
            )
    }

    fn core_application(
        &mut self,
        value: &SignatureTypeKey,
        expected: DefaultOperationExpectedTypeShapeV1,
        site: DefaultBodyOperationTypingSiteV1,
    ) -> Result<DefaultCoreApplicationV1, ExportDefaultBodyOperationTypingValidationError<E>> {
        let actual = self
            .authority
            .classify_default_core_application(value, self.path)
            .map_err(
                |error| ExportDefaultBodyOperationTypingValidationError::Authority { site, error },
            )?;
        match actual {
            Some(actual) if actual.kind() == expected => Ok(actual),
            actual => Err(
                ExportDefaultBodyOperationTypingValidationError::CoreApplication {
                    site,
                    expected,
                    actual: actual.as_ref().map(DefaultCoreApplicationV1::kind),
                },
            ),
        }
    }

    fn entity_shape(
        &mut self,
        entity: DefaultOperationEntityV1<'_>,
        site: DefaultBodyOperationTypingSiteV1,
    ) -> Result<DefaultOperationEntityShapeV1, ExportDefaultBodyOperationTypingValidationError<E>>
    {
        self.authority
            .default_operation_entity_shape(entity, self.path)
            .map_err(
                |error| ExportDefaultBodyOperationTypingValidationError::Authority { site, error },
            )
    }

    fn expect_type(
        &mut self,
        actual: &SignatureTypeKey,
        expected: &SignatureTypeKey,
        site: DefaultBodyOperationTypingSiteV1,
    ) -> Result<(), ExportDefaultBodyOperationTypingValidationError<E>> {
        if actual == expected {
            Ok(())
        } else {
            Err(ExportDefaultBodyOperationTypingValidationError::Type {
                site,
                expected: Box::new(expected.clone()),
                actual: Box::new(actual.clone()),
            })
        }
    }

    fn expect_arity(
        &mut self,
        actual: usize,
        expected: usize,
        site: DefaultBodyOperationTypingSiteV1,
    ) -> Result<(), ExportDefaultBodyOperationTypingValidationError<E>> {
        if actual == expected {
            Ok(())
        } else {
            Err(ExportDefaultBodyOperationTypingValidationError::Arity {
                site,
                expected,
                actual,
            })
        }
    }

    fn expect_mutability(
        &mut self,
        actual: CanonicalBooleanV1,
        expected: CanonicalBooleanV1,
        site: DefaultBodyOperationTypingSiteV1,
    ) -> Result<(), ExportDefaultBodyOperationTypingValidationError<E>> {
        if actual == expected {
            Ok(())
        } else {
            Err(
                ExportDefaultBodyOperationTypingValidationError::Mutability {
                    site,
                    expected,
                    actual,
                },
            )
        }
    }

    fn expect_relation(
        &mut self,
        relation: DefaultOperationTypeRelationV1,
        source: &SignatureTypeKey,
        target: &SignatureTypeKey,
        site: DefaultBodyOperationTypingSiteV1,
    ) -> Result<(), ExportDefaultBodyOperationTypingValidationError<E>> {
        let valid = self
            .authority
            .default_operation_type_relation(relation, source, target, self.path)
            .map_err(
                |error| ExportDefaultBodyOperationTypingValidationError::Authority { site, error },
            )?;
        if valid {
            Ok(())
        } else {
            Err(ExportDefaultBodyOperationTypingValidationError::Relation {
                site,
                relation,
                source: Box::new(source.clone()),
                target: Box::new(target.clone()),
            })
        }
    }

    fn expect_assignable(
        &mut self,
        source: &SignatureTypeKey,
        target: &SignatureTypeKey,
        site: DefaultBodyOperationTypingSiteV1,
    ) -> Result<(), ExportDefaultBodyOperationTypingValidationError<E>> {
        if source == target {
            Ok(())
        } else {
            self.expect_relation(
                DefaultOperationTypeRelationV1::ReferenceRetype,
                source,
                target,
                site,
            )
        }
    }

    fn validate_intrinsic(
        &mut self,
        intrinsic: DefaultOperationIntrinsicV1<'_>,
        site: DefaultBodyOperationTypingSiteV1,
    ) -> Result<(), ExportDefaultBodyOperationTypingValidationError<E>> {
        self.authority
            .validate_default_operation_intrinsic(intrinsic, self.path)
            .map_err(
                |error| ExportDefaultBodyOperationTypingValidationError::Authority { site, error },
            )
    }

    fn callable_shape(
        &mut self,
        entity: DefaultOperationEntityV1<'_>,
        site: DefaultBodyOperationTypingSiteV1,
    ) -> Result<
        super::DefaultCallableOperationShapeV1,
        ExportDefaultBodyOperationTypingValidationError<E>,
    > {
        let shape = self.entity_shape(entity, site)?;
        match shape {
            DefaultOperationEntityShapeV1::Callable(shape) => Ok(shape),
            actual => Err(
                ExportDefaultBodyOperationTypingValidationError::EntityShape {
                    site,
                    expected: DefaultOperationEntityShapeKindV1::Callable,
                    actual: actual.kind(),
                },
            ),
        }
    }

    fn aggregate_shape(
        &mut self,
        entity: DefaultOperationEntityV1<'_>,
        site: DefaultBodyOperationTypingSiteV1,
    ) -> Result<
        super::DefaultAggregateOperationShapeV1,
        ExportDefaultBodyOperationTypingValidationError<E>,
    > {
        let shape = self.entity_shape(entity, site)?;
        match shape {
            DefaultOperationEntityShapeV1::Aggregate(shape) => Ok(shape),
            actual => Err(
                ExportDefaultBodyOperationTypingValidationError::EntityShape {
                    site,
                    expected: DefaultOperationEntityShapeKindV1::Aggregate,
                    actual: actual.kind(),
                },
            ),
        }
    }

    fn field_shape(
        &mut self,
        entity: DefaultOperationEntityV1<'_>,
        site: DefaultBodyOperationTypingSiteV1,
    ) -> Result<
        super::DefaultFieldOperationShapeV1,
        ExportDefaultBodyOperationTypingValidationError<E>,
    > {
        let shape = self.entity_shape(entity, site)?;
        match shape {
            DefaultOperationEntityShapeV1::Field(shape) => Ok(shape),
            actual => Err(
                ExportDefaultBodyOperationTypingValidationError::EntityShape {
                    site,
                    expected: DefaultOperationEntityShapeKindV1::Field,
                    actual: actual.kind(),
                },
            ),
        }
    }

    fn variant_field_shape(
        &mut self,
        entity: DefaultOperationEntityV1<'_>,
        site: DefaultBodyOperationTypingSiteV1,
    ) -> Result<
        super::DefaultVariantFieldOperationShapeV1,
        ExportDefaultBodyOperationTypingValidationError<E>,
    > {
        let shape = self.entity_shape(entity, site)?;
        match shape {
            DefaultOperationEntityShapeV1::VariantField(shape) => Ok(shape),
            actual => Err(
                ExportDefaultBodyOperationTypingValidationError::EntityShape {
                    site,
                    expected: DefaultOperationEntityShapeKindV1::VariantField,
                    actual: actual.kind(),
                },
            ),
        }
    }

    fn value_shape(
        &mut self,
        entity: DefaultOperationEntityV1<'_>,
        site: DefaultBodyOperationTypingSiteV1,
    ) -> Result<
        super::DefaultValueOperationShapeV1,
        ExportDefaultBodyOperationTypingValidationError<E>,
    > {
        let shape = self.entity_shape(entity, site)?;
        match shape {
            DefaultOperationEntityShapeV1::Value(shape) => Ok(shape),
            actual => Err(
                ExportDefaultBodyOperationTypingValidationError::EntityShape {
                    site,
                    expected: DefaultOperationEntityShapeKindV1::Value,
                    actual: actual.kind(),
                },
            ),
        }
    }

    fn type_shape(
        &mut self,
        entity: DefaultOperationEntityV1<'_>,
        site: DefaultBodyOperationTypingSiteV1,
    ) -> Result<SignatureTypeKey, ExportDefaultBodyOperationTypingValidationError<E>> {
        let shape = self.entity_shape(entity, site)?;
        match shape {
            DefaultOperationEntityShapeV1::Type(value) => Ok(value),
            actual => Err(
                ExportDefaultBodyOperationTypingValidationError::EntityShape {
                    site,
                    expected: DefaultOperationEntityShapeKindV1::Type,
                    actual: actual.kind(),
                },
            ),
        }
    }

    fn local_record(
        &mut self,
        selector: &scoop_identity::LocalValueSelector,
        site: DefaultBodyOperationTypingSiteV1,
    ) -> Result<TemplateLocalRecordV1, ExportDefaultBodyOperationTypingValidationError<E>> {
        self.template.locals().get(selector).cloned().ok_or(
            ExportDefaultBodyOperationTypingValidationError::Problem {
                site,
                problem: DefaultBodyOperationTypingProblemV1::MissingLocal,
            },
        )
    }

    fn validate_temporary(
        &mut self,
        temporary: &DefaultBindingTemporaryV1,
        site: DefaultBodyOperationTypingSiteV1,
    ) -> Result<(), ExportDefaultBodyOperationTypingValidationError<E>> {
        let record = self.local_record(temporary.local(), site)?;
        self.expect_type(record.value_type(), temporary.value_type(), site)?;
        self.expect_mutability(record.mutable(), CanonicalBooleanV1::False, site)
    }

    fn validate_callable_without_captures(
        &mut self,
        shape: &super::DefaultCallableOperationShapeV1,
        site: DefaultBodyOperationTypingSiteV1,
    ) -> Result<(), ExportDefaultBodyOperationTypingValidationError<E>> {
        if shape.captures().is_empty() {
            Ok(())
        } else {
            Err(ExportDefaultBodyOperationTypingValidationError::Problem {
                site,
                problem: DefaultBodyOperationTypingProblemV1::UnexpectedCallableCaptures,
            })
        }
    }

    fn problem<T>(
        &self,
        site: DefaultBodyOperationTypingSiteV1,
        problem: DefaultBodyOperationTypingProblemV1,
    ) -> Result<T, ExportDefaultBodyOperationTypingValidationError<E>> {
        Err(ExportDefaultBodyOperationTypingValidationError::Problem { site, problem })
    }
}

enum BodyNode<'a> {
    Body(&'a ExportDefaultBodyV1),
    Statement(&'a DefaultStatementV1),
    Expression(&'a DefaultExpressionV1),
    Assignment {
        target: &'a DefaultAssignTargetV1,
        value: &'a DefaultExpressionV1,
    },
    Pattern {
        pattern: &'a DefaultPatternV1,
        subject: SignatureTypeKey,
    },
    When(&'a DefaultWhenV1),
    WhenArm {
        arm: &'a DefaultWhenArmV1,
        index: usize,
        subject: SignatureTypeKey,
    },
    WhenGuard {
        guard: &'a DefaultWhenGuardV1,
        arm: usize,
    },
    WhenFallback {
        fallback: &'a DefaultWhenFallbackV1,
        subject: SignatureTypeKey,
    },
    Try(&'a DefaultTryV1),
    Catch {
        catch: &'a crate::DefaultCatchV1,
        index: usize,
    },
    For(&'a DefaultForIterationPlanV1),
    BindingPlan(&'a DefaultBindingPlanV1),
    BindingAction {
        action: &'a DefaultBindingActionV1,
        index: usize,
        actions: &'a [DefaultBindingActionV1],
    },
    BindingShape {
        shape: &'a DefaultBindingShapeV1,
        source: &'a DefaultBindingTemporaryV1,
        actions: &'a [DefaultBindingActionV1],
    },
}

fn effect_is_ordinary(effect: Effect) -> bool {
    effect == Effect::Ordinary
}
