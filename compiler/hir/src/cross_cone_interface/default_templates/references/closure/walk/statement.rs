use super::super::ExportDefaultReferenceClosureValidationError;
use super::super::{
    ConstructorTargetView, ExportDefaultReferenceOccurrenceSiteV1, FieldTargetView,
};
use super::{BodyNode, Validator, WorkItem};
use crate::{
    DefaultAssignTargetV1, DefaultBodyProviderTypeSiteV1, DefaultPatternV1, DefaultPatternViewV1,
    DefaultStatementKindV1, DefaultStatementV1, ExportDefinitionSourceV1,
    OptionalDefaultStatementListViewV1,
};

mod bindings;
mod control_flow;

impl Validator<'_> {
    pub(super) fn process_statement<'body>(
        &mut self,
        statement: &'body DefaultStatementV1,
        depth: u64,
        pending: &mut Vec<WorkItem<'body>>,
    ) -> Result<(), ExportDefaultReferenceClosureValidationError> {
        let origin = statement.definition_origin();
        match statement.kind() {
            DefaultStatementKindV1::Expr(expression)
            | DefaultStatementKindV1::Throw(expression) => {
                self.push_child(pending, depth, BodyNode::Expression(expression))
            }
            DefaultStatementKindV1::InitializationEnsure(_)
            | DefaultStatementKindV1::Break
            | DefaultStatementKindV1::Continue => Ok(()),
            DefaultStatementKindV1::LocalFunction(function) => {
                self.push_child(pending, depth, BodyNode::LocalFunction { function, origin })
            }
            DefaultStatementKindV1::Return(value) => match value.as_ref() {
                Some(expression) => {
                    self.push_child(pending, depth, BodyNode::Expression(expression))
                }
                None => Ok(()),
            },
            DefaultStatementKindV1::ValDecl { pattern, init } => {
                self.push_child(pending, depth, BodyNode::Expression(init))?;
                self.push_child(pending, depth, BodyNode::Pattern { pattern, origin })
            }
            DefaultStatementKindV1::Assign { target, value } => {
                self.push_child(pending, depth, BodyNode::Expression(value))?;
                self.push_child(pending, depth, BodyNode::AssignTarget { target, origin })
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
            DefaultStatementKindV1::For(plan) => {
                self.push_child(pending, depth, BodyNode::For { plan, origin })
            }
            DefaultStatementKindV1::When(value) => {
                self.push_child(pending, depth, BodyNode::When { value, origin })
            }
            DefaultStatementKindV1::Try(value) => {
                self.push_child(pending, depth, BodyNode::Try(value))
            }
        }
    }

    pub(super) fn process_pattern<'body>(
        &mut self,
        pattern: &'body DefaultPatternV1,
        origin: &'body ExportDefinitionSourceV1,
        depth: u64,
        pending: &mut Vec<WorkItem<'body>>,
    ) -> Result<(), ExportDefaultReferenceClosureValidationError> {
        match pattern.view() {
            DefaultPatternViewV1::Binding { .. } | DefaultPatternViewV1::Wildcard => Ok(()),
            DefaultPatternViewV1::Literal {
                equality,
                subject_type,
                ..
            } => {
                self.push_type(
                    pending,
                    subject_type,
                    origin,
                    DefaultBodyProviderTypeSiteV1::PatternSubject,
                )?;
                self.push_child(
                    pending,
                    depth,
                    BodyNode::LiteralEquality { equality, origin },
                )
            }
            DefaultPatternViewV1::Variant { variant, fields } => {
                for field in fields.iter().rev() {
                    self.push_child(
                        pending,
                        depth,
                        BodyNode::Pattern {
                            pattern: field.pattern(),
                            origin,
                        },
                    )?;
                }
                self.push_child(
                    pending,
                    depth,
                    BodyNode::ConstructorUse {
                        target: ConstructorTargetView::Variant(variant),
                        origin,
                        site: ExportDefaultReferenceOccurrenceSiteV1::Pattern,
                    },
                )
            }
            DefaultPatternViewV1::Tuple { elements } => {
                self.push_patterns(pending, depth, elements, origin)
            }
            DefaultPatternViewV1::Struct { owner_type, fields } => {
                for field in fields.iter().rev() {
                    self.push_child(
                        pending,
                        depth,
                        BodyNode::Pattern {
                            pattern: field.pattern(),
                            origin,
                        },
                    )?;
                }
                self.push_type(
                    pending,
                    owner_type,
                    origin,
                    DefaultBodyProviderTypeSiteV1::PatternStructOwner,
                )
            }
        }
    }

    fn push_patterns<'body>(
        &mut self,
        pending: &mut Vec<WorkItem<'body>>,
        depth: u64,
        patterns: &'body [DefaultPatternV1],
        origin: &'body ExportDefinitionSourceV1,
    ) -> Result<(), ExportDefaultReferenceClosureValidationError> {
        for pattern in patterns.iter().rev() {
            self.push_child(pending, depth, BodyNode::Pattern { pattern, origin })?;
        }
        Ok(())
    }

    pub(super) fn process_assign_target<'body>(
        &mut self,
        target: &'body DefaultAssignTargetV1,
        origin: &'body ExportDefinitionSourceV1,
        depth: u64,
        pending: &mut Vec<WorkItem<'body>>,
    ) -> Result<(), ExportDefaultReferenceClosureValidationError> {
        match target {
            DefaultAssignTargetV1::Local { .. } => Ok(()),
            DefaultAssignTargetV1::Global { property } => self.push_global(
                pending,
                *property,
                origin,
                ExportDefaultReferenceOccurrenceSiteV1::Assignment,
            ),
            DefaultAssignTargetV1::Index { array, index } => {
                self.push_child(pending, depth, BodyNode::Expression(index))?;
                self.push_child(pending, depth, BodyNode::Expression(array))
            }
            DefaultAssignTargetV1::Field { receiver, field } => {
                self.push_child(
                    pending,
                    depth,
                    BodyNode::FieldUse {
                        target: FieldTargetView::Field(field),
                        origin,
                        site: ExportDefaultReferenceOccurrenceSiteV1::Assignment,
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
    ) -> Result<(), ExportDefaultReferenceClosureValidationError> {
        for statement in statements.iter().rev() {
            self.push_child(pending, depth, BodyNode::Statement(statement))?;
        }
        Ok(())
    }
}
