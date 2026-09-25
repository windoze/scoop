use crate::{
    CanonicalBooleanV1, DefaultAssignTargetV1, DefaultBodyOperationAuthority,
    DefaultFieldOperationKindV1, DefaultFieldRefV1, DefaultOperationCoreTypeV1,
    DefaultOperationEntityV1, DefaultOperationExpectedTypeShapeV1, DefaultOperationTypeRelationV1,
    DefaultOperationValueRoleV1, DefaultStatementKindV1, DefaultStatementV1,
    DefaultWhenFallbackViewV1, OptionalDefaultStatementListViewV1,
};

use super::{
    BodyNode, BodyValidator, DefaultAssignmentOperationV1, DefaultBodyOperationTypingProblemV1,
    DefaultBodyOperationV1, DefaultStatementOperationV1,
    ExportDefaultBodyOperationTypingValidationError,
};

impl<A, E> BodyValidator<'_, A, E>
where
    A: DefaultBodyOperationAuthority<E>,
{
    pub(super) fn process_statement<'body>(
        &mut self,
        statement: &'body DefaultStatementV1,
        pending: &mut Vec<BodyNode<'body>>,
    ) -> Result<(), ExportDefaultBodyOperationTypingValidationError<E>> {
        match statement.kind() {
            DefaultStatementKindV1::Expr(expression) => {
                self.push_node(pending, BodyNode::Expression(expression))
            }
            DefaultStatementKindV1::InitializationEnsure(_)
            | DefaultStatementKindV1::LocalFunction(_)
            | DefaultStatementKindV1::Break
            | DefaultStatementKindV1::Continue => Ok(()),
            DefaultStatementKindV1::Return(value) => {
                let site = Self::site(
                    DefaultBodyOperationV1::Statement(DefaultStatementOperationV1::Return),
                    DefaultOperationValueRoleV1::ReturnValue,
                );
                match value.as_ref() {
                    Some(value) => {
                        self.expect_type(value.result_type(), self.template.result(), site)?;
                        self.push_node(pending, BodyNode::Expression(value))
                    }
                    None => {
                        let unit = self.core_type(DefaultOperationCoreTypeV1::Unit, site)?;
                        if self.template.result() == &unit {
                            Ok(())
                        } else {
                            self.problem(
                                site,
                                DefaultBodyOperationTypingProblemV1::MissingReturnValue,
                            )
                        }
                    }
                }
            }
            DefaultStatementKindV1::ValDecl { pattern, init } => {
                self.push_node(
                    pending,
                    BodyNode::Pattern {
                        pattern,
                        subject: init.result_type().clone(),
                    },
                )?;
                self.push_node(pending, BodyNode::Expression(init))
            }
            DefaultStatementKindV1::Assign { target, value } => {
                self.push_node(pending, BodyNode::Assignment { target, value })
            }
            DefaultStatementKindV1::If {
                condition,
                then_body,
                else_body,
            } => {
                let site = Self::site(
                    DefaultBodyOperationV1::Statement(DefaultStatementOperationV1::IfCondition),
                    DefaultOperationValueRoleV1::Condition,
                );
                let boolean = self.core_type(DefaultOperationCoreTypeV1::Boolean, site)?;
                self.expect_type(condition.result_type(), &boolean, site)?;
                if let OptionalDefaultStatementListViewV1::Present(else_body) = else_body.view() {
                    self.push_statements(pending, else_body)?;
                }
                self.push_statements(pending, then_body)?;
                self.push_node(pending, BodyNode::Expression(condition))
            }
            DefaultStatementKindV1::While {
                condition_setup,
                condition,
                body,
            } => {
                let site = Self::site(
                    DefaultBodyOperationV1::Statement(DefaultStatementOperationV1::WhileCondition),
                    DefaultOperationValueRoleV1::Condition,
                );
                let boolean = self.core_type(DefaultOperationCoreTypeV1::Boolean, site)?;
                self.expect_type(condition.result_type(), &boolean, site)?;
                self.push_statements(pending, body)?;
                self.push_node(pending, BodyNode::Expression(condition))?;
                self.push_statements(pending, condition_setup)
            }
            DefaultStatementKindV1::For(plan) => self.push_node(pending, BodyNode::For(plan)),
            DefaultStatementKindV1::When(value) => self.push_node(pending, BodyNode::When(value)),
            DefaultStatementKindV1::Try(value) => self.push_node(pending, BodyNode::Try(value)),
            DefaultStatementKindV1::Throw(value) => {
                let site = Self::site(
                    DefaultBodyOperationV1::Statement(DefaultStatementOperationV1::Throw),
                    DefaultOperationValueRoleV1::Operand,
                );
                let throwable = self.core_type(DefaultOperationCoreTypeV1::Throwable, site)?;
                self.expect_assignable(value.result_type(), &throwable, site)?;
                self.push_node(pending, BodyNode::Expression(value))
            }
        }
    }

    pub(super) fn process_assignment<'body>(
        &mut self,
        target: &'body DefaultAssignTargetV1,
        value: &'body crate::DefaultExpressionV1,
        pending: &mut Vec<BodyNode<'body>>,
    ) -> Result<(), ExportDefaultBodyOperationTypingValidationError<E>> {
        match target {
            DefaultAssignTargetV1::Local { local } => {
                let site = Self::site(
                    DefaultBodyOperationV1::Assignment(DefaultAssignmentOperationV1::Local),
                    DefaultOperationValueRoleV1::Target,
                );
                let record = self.local_record(local, site)?;
                self.expect_type(value.result_type(), record.value_type(), site)?;
                self.push_node(pending, BodyNode::Expression(value))
            }
            DefaultAssignTargetV1::Global { property } => {
                let site = Self::site(
                    DefaultBodyOperationV1::Assignment(DefaultAssignmentOperationV1::Global),
                    DefaultOperationValueRoleV1::Target,
                );
                let shape = self.value_shape(DefaultOperationEntityV1::Global(*property), site)?;
                if shape.mutable() != CanonicalBooleanV1::True {
                    return self.problem(
                        site,
                        DefaultBodyOperationTypingProblemV1::ImmutableAssignmentTarget,
                    );
                }
                self.expect_type(value.result_type(), shape.value_type(), site)?;
                self.push_node(pending, BodyNode::Expression(value))
            }
            DefaultAssignTargetV1::Index { array, index } => {
                let operation =
                    DefaultBodyOperationV1::Assignment(DefaultAssignmentOperationV1::Index);
                let receiver_site = Self::site(operation, DefaultOperationValueRoleV1::Receiver);
                let application = self.core_application(
                    array.result_type(),
                    DefaultOperationExpectedTypeShapeV1::MutableArray,
                    receiver_site,
                )?;
                let long = self.core_type(
                    DefaultOperationCoreTypeV1::Integer(crate::DefaultIntegerKindV1::Signed64),
                    Self::site(operation, DefaultOperationValueRoleV1::Index),
                )?;
                self.expect_type(
                    index.result_type(),
                    &long,
                    Self::site(operation, DefaultOperationValueRoleV1::Index),
                )?;
                self.expect_type(
                    value.result_type(),
                    application.element(),
                    Self::site(operation, DefaultOperationValueRoleV1::Value),
                )?;
                self.push_node(pending, BodyNode::Expression(value))?;
                self.push_node(pending, BodyNode::Expression(index))?;
                self.push_node(pending, BodyNode::Expression(array))
            }
            DefaultAssignTargetV1::Field { receiver, field } => {
                let operation =
                    DefaultBodyOperationV1::Assignment(DefaultAssignmentOperationV1::Field);
                let target_site = Self::site(operation, DefaultOperationValueRoleV1::Target);
                let owner_type = match field {
                    DefaultFieldRefV1::Class { owner_type, .. } => owner_type,
                    DefaultFieldRefV1::Struct { .. } | DefaultFieldRefV1::Tuple { .. } => {
                        return self.problem(
                            target_site,
                            DefaultBodyOperationTypingProblemV1::InvalidAssignmentTarget,
                        );
                    }
                };
                let shape =
                    self.field_shape(DefaultOperationEntityV1::Field(field), target_site)?;
                if shape.kind() != DefaultFieldOperationKindV1::Class {
                    return self.problem(
                        target_site,
                        DefaultBodyOperationTypingProblemV1::InvalidAssignmentTarget,
                    );
                }
                self.expect_type(shape.owner_type(), owner_type, target_site)?;
                self.expect_relation(
                    DefaultOperationTypeRelationV1::MemberReceiver,
                    receiver.result_type(),
                    shape.owner_type(),
                    Self::site(operation, DefaultOperationValueRoleV1::Receiver),
                )?;
                if shape.mutable() != CanonicalBooleanV1::True {
                    return self.problem(
                        target_site,
                        DefaultBodyOperationTypingProblemV1::ImmutableAssignmentTarget,
                    );
                }
                self.expect_type(
                    value.result_type(),
                    shape.value_type(),
                    Self::site(operation, DefaultOperationValueRoleV1::Value),
                )?;
                self.push_node(pending, BodyNode::Expression(value))?;
                self.push_node(pending, BodyNode::Expression(receiver))
            }
        }
    }

    pub(super) fn process_when<'body>(
        &mut self,
        value: &'body crate::DefaultWhenV1,
        pending: &mut Vec<BodyNode<'body>>,
    ) -> Result<(), ExportDefaultBodyOperationTypingValidationError<E>> {
        let subject = value.subject().result_type().clone();
        self.push_node(
            pending,
            BodyNode::WhenFallback {
                fallback: value.fallback(),
                subject: subject.clone(),
            },
        )?;
        for (index, arm) in value.arms().iter().enumerate().rev() {
            self.push_node(
                pending,
                BodyNode::WhenArm {
                    arm,
                    index,
                    subject: subject.clone(),
                },
            )?;
        }
        self.push_node(pending, BodyNode::Expression(value.subject()))
    }

    pub(super) fn process_when_arm<'body>(
        &mut self,
        arm: &'body crate::DefaultWhenArmV1,
        index: usize,
        subject: &SignatureTypeKey,
        pending: &mut Vec<BodyNode<'body>>,
    ) -> Result<(), ExportDefaultBodyOperationTypingValidationError<E>> {
        self.push_statements(pending, arm.body())?;
        if let Some(guard) = arm.guard().as_ref() {
            self.push_node(pending, BodyNode::WhenGuard { guard, arm: index })?;
        }
        self.push_node(
            pending,
            BodyNode::Pattern {
                pattern: arm.pattern(),
                subject: subject.clone(),
            },
        )
    }

    pub(super) fn process_when_guard<'body>(
        &mut self,
        guard: &'body crate::DefaultWhenGuardV1,
        arm: usize,
        pending: &mut Vec<BodyNode<'body>>,
    ) -> Result<(), ExportDefaultBodyOperationTypingValidationError<E>> {
        let site = Self::site(
            DefaultBodyOperationV1::Statement(DefaultStatementOperationV1::WhenGuard { arm }),
            DefaultOperationValueRoleV1::Condition,
        );
        let boolean = self.core_type(DefaultOperationCoreTypeV1::Boolean, site)?;
        self.expect_type(guard.condition().result_type(), &boolean, site)?;
        self.push_node(pending, BodyNode::Expression(guard.condition()))?;
        self.push_statements(pending, guard.setup())
    }

    pub(super) fn process_when_fallback<'body>(
        &mut self,
        fallback: &'body crate::DefaultWhenFallbackV1,
        subject: &SignatureTypeKey,
        pending: &mut Vec<BodyNode<'body>>,
    ) -> Result<(), ExportDefaultBodyOperationTypingValidationError<E>> {
        let operation =
            DefaultBodyOperationV1::Statement(DefaultStatementOperationV1::WhenFallback);
        match fallback.view() {
            DefaultWhenFallbackViewV1::Else(statements) => {
                self.push_statements(pending, statements)
            }
            DefaultWhenFallbackViewV1::IrrefutableArm { subject_type }
            | DefaultWhenFallbackViewV1::PatternMatrix { subject_type } => self.expect_type(
                subject_type,
                subject,
                Self::site(operation, DefaultOperationValueRoleV1::Subject),
            ),
            DefaultWhenFallbackViewV1::EnumPatternMatrix {
                subject_type,
                owner_type,
            } => {
                let subject_site = Self::site(operation, DefaultOperationValueRoleV1::Subject);
                self.expect_type(subject_type, subject, subject_site)?;
                self.expect_type(owner_type, subject, subject_site)?;
                let canonical = self.type_shape(
                    DefaultOperationEntityV1::Enum(owner_type),
                    Self::site(operation, DefaultOperationValueRoleV1::Target),
                )?;
                self.expect_type(owner_type, &canonical, subject_site)
            }
        }
    }

    pub(super) fn process_try<'body>(
        &mut self,
        value: &'body crate::DefaultTryV1,
        pending: &mut Vec<BodyNode<'body>>,
    ) -> Result<(), ExportDefaultBodyOperationTypingValidationError<E>> {
        if let OptionalDefaultStatementListViewV1::Present(finally_body) =
            value.finally_body().view()
        {
            self.push_statements(pending, finally_body)?;
        }
        for (index, catch) in value.catches().iter().enumerate().rev() {
            self.push_node(pending, BodyNode::Catch { catch, index })?;
        }
        self.push_statements(pending, value.body())
    }

    pub(super) fn process_catch<'body>(
        &mut self,
        catch: &'body crate::DefaultCatchV1,
        index: usize,
        pending: &mut Vec<BodyNode<'body>>,
    ) -> Result<(), ExportDefaultBodyOperationTypingValidationError<E>> {
        let site = Self::site(
            DefaultBodyOperationV1::Statement(DefaultStatementOperationV1::Catch { index }),
            DefaultOperationValueRoleV1::Target,
        );
        let record = self.local_record(catch.local(), site)?;
        self.expect_type(record.value_type(), catch.value_type(), site)?;
        self.expect_mutability(record.mutable(), CanonicalBooleanV1::False, site)?;
        let throwable = self.core_type(DefaultOperationCoreTypeV1::Throwable, site)?;
        self.expect_assignable(catch.value_type(), &throwable, site)?;
        self.push_statements(pending, catch.body())
    }
}
use scoop_identity::SignatureTypeKey;
