//! Common initialization uses a separate physical local-value scope.

use super::*;

impl Concretizer<'_> {
    pub(super) fn append_common_initialization(
        &mut self,
        body: &mut concrete::Body,
        common: &[export::ClassInitializationStep],
        substitution: &[concrete::TypeId],
        origin: export::ConcreteExpressionOrigin,
    ) {
        let previous = self.evaluation_context.replace(origin.evaluation.context);
        for step in common {
            match step {
                export::ClassInitializationStep::Field {
                    field,
                    initializer,
                    span,
                } => {
                    let locals = self.append_common_locals(body, &initializer.locals, substitution);
                    body.statements.extend(self.lower_statement_region(
                        &initializer.statements,
                        substitution,
                        &locals,
                    ));
                    let value = self.lower_expr(&initializer.value, substitution, &locals);
                    let (receiver_ty, field) =
                        self.lower_initializing_class_field(*field, substitution);
                    let receiver = self.constructor_receiver(receiver_ty, *span, origin);
                    body.statements.push(concrete::Statement {
                        kind: concrete::StatementKind::Assign {
                            target: concrete::AssignTarget::Field {
                                receiver: Box::new(receiver),
                                field,
                            },
                            value,
                        },
                        span: *span,
                    });
                }
                export::ClassInitializationStep::InitBlock {
                    body: source_body, ..
                } => {
                    let locals = self.append_common_locals(body, &source_body.locals, substitution);
                    body.statements.extend(self.lower_statement_region(
                        &source_body.statements,
                        substitution,
                        &locals,
                    ));
                }
            }
        }
        self.evaluation_context = previous;
    }

    fn append_common_locals(
        &mut self,
        body: &mut concrete::Body,
        source: &Arena<export::Local>,
        substitution: &[concrete::TypeId],
    ) -> Vec<concrete::LocalId> {
        let locals = self.append_source_locals(body, source, substitution);
        for local in &locals {
            use scoop_identity::LocalValueSelector;
            let selector = &mut body.locals[*local].selector;
            let path = match selector {
                LocalValueSelector::LocalDeclaration { path }
                | LocalValueSelector::BoundReceiver { path }
                | LocalValueSelector::Synthetic { path, .. } => path,
                LocalValueSelector::SuspensionResult { site } => site,
                LocalValueSelector::This | LocalValueSelector::Parameter { .. } => continue,
            };
            // A common fragment is executed in each terminal constructor;
            // its physical locals cannot overlap that constructor's own locals.
            *path = scoop_identity::StructuralDefinitionPath::from_first(
                scoop_identity::StructuralPathSegment::new(
                    scoop_identity::StructuralDefinitionSiteRole::Initializer,
                    0,
                ),
                path.segments().iter().copied(),
            );
        }
        locals
    }
}
