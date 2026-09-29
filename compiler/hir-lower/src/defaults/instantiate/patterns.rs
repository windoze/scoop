use super::*;

impl Lowerer {
    pub(super) fn instantiate_default_assign_target(
        &mut self,
        source: &hir::AssignTarget,
        context: &mut InstantiationContext,
    ) -> hir::AssignTarget {
        match source {
            hir::AssignTarget::Local(local) => {
                hir::AssignTarget::Local(mapped_local(context, *local))
            }
            hir::AssignTarget::Global(global) => hir::AssignTarget::Global(*global),
            hir::AssignTarget::GenericDelegateStorage(reference) => {
                hir::AssignTarget::GenericDelegateStorage(
                    self.instantiate_delegate_reference(reference, context),
                )
            }
            hir::AssignTarget::SingletonPublishedRoot(root) => {
                hir::AssignTarget::SingletonPublishedRoot(*root)
            }
            hir::AssignTarget::Index { array, index } => hir::AssignTarget::Index {
                array: Box::new(self.instantiate_default_expr(array, context)),
                index: Box::new(self.instantiate_default_expr(index, context)),
            },
            hir::AssignTarget::Field { receiver, field } => hir::AssignTarget::Field {
                receiver: Box::new(self.instantiate_default_expr(receiver, context)),
                field: self.instantiate_default_field(*field, context),
            },
            hir::AssignTarget::InitializingClassField { field, origin } => {
                hir::AssignTarget::InitializingClassField {
                    field: self.instantiate_default_initializing_field(*field, context),
                    origin: instantiate_origin(*origin, context.evaluation),
                }
            }
        }
    }

    pub(super) fn instantiate_default_pattern(
        &mut self,
        source: &hir::Pattern,
        context: &mut InstantiationContext,
    ) -> hir::Pattern {
        match source {
            hir::Pattern::Binding { local } => hir::Pattern::Binding {
                local: mapped_local(context, *local),
            },
            hir::Pattern::Wildcard => hir::Pattern::Wildcard,
            hir::Pattern::Literal {
                value,
                equality,
                subject_ty,
            } => hir::Pattern::Literal {
                value: self.instantiate_default_expr(value, context),
                equality: match *equality {
                    hir::LiteralPatternEquality::Integer { kind } => {
                        hir::LiteralPatternEquality::Integer { kind }
                    }
                    hir::LiteralPatternEquality::Ordinary { equals } => {
                        hir::LiteralPatternEquality::Ordinary {
                            equals: self.instantiate_default_callable(equals, context),
                        }
                    }
                },
                subject_ty: self.instantiate_method_ty(*subject_ty, &context.bindings),
            },
            hir::Pattern::Variant {
                application,
                fields,
            } => hir::Pattern::Variant {
                application: self.instantiate_default_variant(*application, context),
                fields: fields
                    .iter()
                    .map(|(index, pattern)| {
                        (*index, self.instantiate_default_pattern(pattern, context))
                    })
                    .collect(),
            },
            hir::Pattern::Tuple(elements) => hir::Pattern::Tuple(
                elements
                    .iter()
                    .map(|element| self.instantiate_default_pattern(element, context))
                    .collect(),
            ),
            hir::Pattern::Struct { owner, fields } => hir::Pattern::Struct {
                owner: self.instantiate_method_ty(*owner, &context.bindings),
                fields: fields
                    .iter()
                    .map(|(index, pattern)| {
                        (*index, self.instantiate_default_pattern(pattern, context))
                    })
                    .collect(),
            },
        }
    }
}
