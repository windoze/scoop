use crate::{
    DefaultAssignTargetV1, DefaultPatternV1, DefaultPatternViewV1, DefaultStatementKindV1,
    DefaultStatementV1, ExportDefinitionSourceV1, OptionalDefaultStatementListViewV1,
};

use super::{BodyEnvelopeAuthority, BodyNode, Validator, WorkItem};
use crate::{
    DefaultBodyOriginSiteV1, DefaultBodyProviderEnvelopeSemanticValidationError,
    DefaultBodyProviderTypeSiteV1,
};

mod bindings;
mod control_flow;

impl<A, E> Validator<'_, A, E>
where
    A: BodyEnvelopeAuthority<E>,
{
    pub(super) fn process_statement<'body>(
        &mut self,
        statement: &'body DefaultStatementV1,
        depth: u64,
        pending: &mut Vec<WorkItem<'body>>,
    ) -> Result<(), DefaultBodyProviderEnvelopeSemanticValidationError<E>> {
        self.process_statement_kind(
            statement.kind(),
            statement.definition_origin(),
            depth,
            pending,
        )?;
        self.push_child(
            pending,
            depth,
            BodyNode::Origin {
                source: statement.definition_origin(),
                site: DefaultBodyOriginSiteV1::Statement,
            },
        )
    }

    fn process_statement_kind<'body>(
        &mut self,
        kind: &'body DefaultStatementKindV1,
        definition_origin: &'body ExportDefinitionSourceV1,
        depth: u64,
        pending: &mut Vec<WorkItem<'body>>,
    ) -> Result<(), DefaultBodyProviderEnvelopeSemanticValidationError<E>> {
        match kind {
            DefaultStatementKindV1::Expr(expression)
            | DefaultStatementKindV1::Throw(expression) => {
                self.push_child(pending, depth, BodyNode::Expression(expression))
            }
            DefaultStatementKindV1::InitializationEnsure(_)
            | DefaultStatementKindV1::Break
            | DefaultStatementKindV1::Continue => Ok(()),
            DefaultStatementKindV1::LocalFunction(function) => self.push_child(
                pending,
                depth,
                BodyNode::LocalFunction {
                    function,
                    definition_origin,
                },
            ),
            DefaultStatementKindV1::Return(value) => match value.as_ref() {
                Some(expression) => {
                    self.push_child(pending, depth, BodyNode::Expression(expression))
                }
                None => Ok(()),
            },
            DefaultStatementKindV1::ValDecl { pattern, init } => {
                self.push_child(pending, depth, BodyNode::Expression(init))?;
                self.push_child(
                    pending,
                    depth,
                    BodyNode::Pattern {
                        pattern,
                        definition_origin,
                    },
                )
            }
            DefaultStatementKindV1::Assign { target, value } => {
                self.push_child(pending, depth, BodyNode::Expression(value))?;
                self.push_child(
                    pending,
                    depth,
                    BodyNode::AssignTarget {
                        target,
                        definition_origin,
                    },
                )
            }
            DefaultStatementKindV1::If {
                condition,
                then_body,
                else_body,
            } => {
                if let OptionalDefaultStatementListViewV1::Present(statements) = else_body.view() {
                    self.push_statements(pending, depth, statements)?;
                }
                self.push_statements(pending, depth, then_body)?;
                self.push_child(pending, depth, BodyNode::Expression(condition))
            }
            DefaultStatementKindV1::While {
                condition_setup,
                condition,
                body,
            } => {
                self.push_statements(pending, depth, body)?;
                self.push_child(pending, depth, BodyNode::Expression(condition))?;
                self.push_statements(pending, depth, condition_setup)
            }
            DefaultStatementKindV1::For(plan) => self.push_child(
                pending,
                depth,
                BodyNode::For {
                    plan,
                    definition_origin,
                },
            ),
            DefaultStatementKindV1::When(value) => self.push_child(
                pending,
                depth,
                BodyNode::When {
                    value,
                    definition_origin,
                },
            ),
            DefaultStatementKindV1::Try(value) => self.push_child(
                pending,
                depth,
                BodyNode::Try {
                    value,
                    definition_origin,
                },
            ),
        }
    }

    pub(super) fn process_pattern<'body>(
        &mut self,
        pattern: &'body DefaultPatternV1,
        definition_origin: &'body ExportDefinitionSourceV1,
        depth: u64,
        pending: &mut Vec<WorkItem<'body>>,
    ) -> Result<(), DefaultBodyProviderEnvelopeSemanticValidationError<E>> {
        match pattern.view() {
            DefaultPatternViewV1::Binding { .. } | DefaultPatternViewV1::Wildcard => Ok(()),
            DefaultPatternViewV1::Literal {
                equality,
                subject_type,
                ..
            } => {
                self.push_child(
                    pending,
                    depth,
                    BodyNode::LiteralEquality {
                        equality,
                        definition_origin,
                    },
                )?;
                self.push_type(
                    pending,
                    subject_type,
                    DefaultBodyProviderTypeSiteV1::PatternSubject,
                    definition_origin,
                )
            }
            DefaultPatternViewV1::Variant { variant, fields } => {
                for field in fields.iter().rev() {
                    self.push_child(
                        pending,
                        depth,
                        BodyNode::Pattern {
                            pattern: field.pattern(),
                            definition_origin,
                        },
                    )?;
                }
                self.push_child(
                    pending,
                    depth,
                    BodyNode::EnumVariantRef {
                        variant,
                        definition_origin,
                    },
                )
            }
            DefaultPatternViewV1::Tuple { elements } => {
                self.push_patterns(pending, depth, elements, definition_origin)
            }
            DefaultPatternViewV1::Struct { owner_type, fields } => {
                for field in fields.iter().rev() {
                    self.push_child(
                        pending,
                        depth,
                        BodyNode::Pattern {
                            pattern: field.pattern(),
                            definition_origin,
                        },
                    )?;
                }
                self.push_type(
                    pending,
                    owner_type,
                    DefaultBodyProviderTypeSiteV1::PatternStructOwner,
                    definition_origin,
                )
            }
        }
    }

    fn push_patterns<'body>(
        &mut self,
        pending: &mut Vec<WorkItem<'body>>,
        depth: u64,
        patterns: &'body [DefaultPatternV1],
        definition_origin: &'body ExportDefinitionSourceV1,
    ) -> Result<(), DefaultBodyProviderEnvelopeSemanticValidationError<E>> {
        for pattern in patterns.iter().rev() {
            self.push_child(
                pending,
                depth,
                BodyNode::Pattern {
                    pattern,
                    definition_origin,
                },
            )?;
        }
        Ok(())
    }

    pub(super) fn process_assign_target<'body>(
        &mut self,
        target: &'body DefaultAssignTargetV1,
        definition_origin: &'body ExportDefinitionSourceV1,
        depth: u64,
        pending: &mut Vec<WorkItem<'body>>,
    ) -> Result<(), DefaultBodyProviderEnvelopeSemanticValidationError<E>> {
        match target {
            DefaultAssignTargetV1::Local { .. } | DefaultAssignTargetV1::Global { .. } => Ok(()),
            DefaultAssignTargetV1::Index { array, index } => {
                self.push_child(pending, depth, BodyNode::Expression(index))?;
                self.push_child(pending, depth, BodyNode::Expression(array))
            }
            DefaultAssignTargetV1::Field { receiver, field } => {
                self.push_child(
                    pending,
                    depth,
                    BodyNode::FieldRef {
                        field,
                        definition_origin,
                    },
                )?;
                self.push_child(pending, depth, BodyNode::Expression(receiver))
            }
        }
    }

    pub(super) fn push_statements<'body>(
        &mut self,
        pending: &mut Vec<WorkItem<'body>>,
        depth: u64,
        statements: &'body [DefaultStatementV1],
    ) -> Result<(), DefaultBodyProviderEnvelopeSemanticValidationError<E>> {
        for statement in statements.iter().rev() {
            self.push_child(pending, depth, BodyNode::Statement(statement))?;
        }
        Ok(())
    }
}
