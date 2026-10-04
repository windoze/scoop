use super::super::{ExportDefaultReferenceOccurrenceSiteV1, FieldTargetView};
use super::{BodyNode, DefaultBodyReferenceVisitorV1, ReferenceWalker, ScheduledWork};
use crate::{
    DefaultAssignTargetV1, DefaultBodyProviderTypeSiteV1, DefaultPatternV1, DefaultPatternViewV1,
    DefaultStatementKindV1, DefaultStatementV1, ExportDefinitionSourceV1,
    OptionalDefaultStatementListViewV1,
};

mod control_flow;

impl<'body, V: DefaultBodyReferenceVisitorV1<'body>> ReferenceWalker<'_, 'body, V> {
    pub(super) fn process_statement(
        &mut self,
        statement: &'body DefaultStatementV1,
        pending: &mut Vec<ScheduledWork<'body>>,
    ) -> Result<(), V::Error> {
        let origin = statement.definition_origin();
        match statement.kind() {
            DefaultStatementKindV1::ContextScope { value, body } => {
                self.push_statements(pending, body)?;
                self.push_child(pending, BodyNode::Expression(value))
            }
            DefaultStatementKindV1::GenericDelegateEnsure(reference) => self.push_generic_delegate(
                pending,
                reference,
                origin,
                ExportDefaultReferenceOccurrenceSiteV1::Statement,
            ),
            DefaultStatementKindV1::Expr(expression)
            | DefaultStatementKindV1::Throw(expression) => {
                self.push_child(pending, BodyNode::Expression(expression))
            }
            DefaultStatementKindV1::InitializationEnsure(_)
            | DefaultStatementKindV1::Break
            | DefaultStatementKindV1::Continue => Ok(()),
            DefaultStatementKindV1::LocalFunction(function) => {
                self.push_child(pending, BodyNode::LocalFunction { function, origin })
            }
            DefaultStatementKindV1::Return(value) => match value.as_ref() {
                Some(expression) => self.push_child(pending, BodyNode::Expression(expression)),
                None => Ok(()),
            },
            DefaultStatementKindV1::ValDecl { pattern, init } => {
                self.push_child(pending, BodyNode::Expression(init))?;
                self.push_child(pending, BodyNode::Pattern { pattern, origin })
            }
            DefaultStatementKindV1::Assign { target, value } => {
                self.push_child(pending, BodyNode::Expression(value))?;
                self.push_child(pending, BodyNode::AssignTarget { target, origin })
            }
            DefaultStatementKindV1::If {
                condition,
                then_body,
                else_body,
            } => {
                if let OptionalDefaultStatementListViewV1::Present(statements) = else_body.view() {
                    self.push_statements(pending, statements)?;
                }
                self.push_statements(pending, then_body)?;
                self.push_child(pending, BodyNode::Expression(condition))
            }
            DefaultStatementKindV1::While {
                condition_setup,
                condition,
                body,
            } => {
                self.push_statements(pending, body)?;
                self.push_child(pending, BodyNode::Expression(condition))?;
                self.push_statements(pending, condition_setup)
            }
            DefaultStatementKindV1::When(value) => {
                self.push_child(pending, BodyNode::When { value, origin })
            }
            DefaultStatementKindV1::Try(value) => self.push_child(pending, BodyNode::Try(value)),
        }
    }

    pub(super) fn process_pattern(
        &mut self,
        pattern: &'body DefaultPatternV1,
        origin: &'body ExportDefinitionSourceV1,
        pending: &mut Vec<ScheduledWork<'body>>,
    ) -> Result<(), V::Error> {
        match pattern.view() {
            DefaultPatternViewV1::Binding { .. } | DefaultPatternViewV1::Wildcard => Ok(()),
            DefaultPatternViewV1::Literal {
                equality,
                subject_type,
                value,
            } => {
                let origin = value.definition_origin();
                self.push_child(pending, BodyNode::Expression(value))?;
                self.push_type(
                    pending,
                    subject_type,
                    origin,
                    DefaultBodyProviderTypeSiteV1::PatternSubject,
                )?;
                self.push_child(pending, BodyNode::LiteralEquality { equality, origin })
            }
            DefaultPatternViewV1::Variant { variant, fields } => {
                for field in fields.iter().rev() {
                    self.push_child(
                        pending,
                        BodyNode::Pattern {
                            pattern: field.pattern(),
                            origin,
                        },
                    )?;
                }
                self.push_type(
                    pending,
                    variant.owner_type(),
                    origin,
                    DefaultBodyProviderTypeSiteV1::PatternSubject,
                )
            }
            DefaultPatternViewV1::Tuple { elements } => {
                self.push_patterns(pending, elements, origin)
            }
            DefaultPatternViewV1::Struct { owner_type, fields } => {
                for field in fields.iter().rev() {
                    self.push_child(
                        pending,
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

    fn push_patterns(
        &mut self,
        pending: &mut Vec<ScheduledWork<'body>>,
        patterns: &'body [DefaultPatternV1],
        origin: &'body ExportDefinitionSourceV1,
    ) -> Result<(), V::Error> {
        for pattern in patterns.iter().rev() {
            self.push_child(pending, BodyNode::Pattern { pattern, origin })?;
        }
        Ok(())
    }

    pub(super) fn process_assign_target(
        &mut self,
        target: &'body DefaultAssignTargetV1,
        origin: &'body ExportDefinitionSourceV1,
        pending: &mut Vec<ScheduledWork<'body>>,
    ) -> Result<(), V::Error> {
        match target {
            DefaultAssignTargetV1::GenericDelegateStorage(reference) => self.push_generic_delegate(
                pending,
                reference,
                origin,
                ExportDefaultReferenceOccurrenceSiteV1::Assignment,
            ),
            DefaultAssignTargetV1::Local { .. } => Ok(()),
            DefaultAssignTargetV1::Global { property } => self.push_global(
                pending,
                *property,
                origin,
                ExportDefaultReferenceOccurrenceSiteV1::Assignment,
            ),
            DefaultAssignTargetV1::Index { array, index } => {
                self.push_child(pending, BodyNode::Expression(index))?;
                self.push_child(pending, BodyNode::Expression(array))
            }
            DefaultAssignTargetV1::Field { receiver, field } => {
                self.push_child(
                    pending,
                    BodyNode::FieldUse {
                        target: FieldTargetView::Field(field),
                        origin,
                        site: ExportDefaultReferenceOccurrenceSiteV1::Assignment,
                    },
                )?;
                self.push_child(pending, BodyNode::Expression(receiver))
            }
        }
    }

    pub(super) fn push_statements(
        &mut self,
        pending: &mut Vec<ScheduledWork<'body>>,
        statements: &'body [DefaultStatementV1],
    ) -> Result<(), V::Error> {
        for statement in statements.iter().rev() {
            self.push_child(pending, BodyNode::Statement(statement))?;
        }
        Ok(())
    }
}
