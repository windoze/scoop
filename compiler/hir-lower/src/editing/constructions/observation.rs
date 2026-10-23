use super::*;
use crate::editing::bodies::CallInput;
use hir::SourceContextConstructor as Constructor;

impl Lowerer {
    pub(in crate::editing) fn constructor_delegation_input(
        &self,
        constructor: Constructor,
        span: ast::Span,
    ) -> Option<CallInput> {
        let inputs = self.editing_constructions.as_ref()?;
        if let Constructor::Class(id) = constructor
            && matches!(
                self.class_constructors[id].kind,
                hir::ClassConstructorKind::Primary { .. }
            )
        {
            let base = inputs.bases.get(&self.class_constructors[id].owner)?;
            if base.span != span {
                return None;
            }
            let arguments = base.constructor_arguments()?;
            let name = match &base.ty.kind {
                ast::TypeRefKind::Named(name)
                | ast::TypeRefKind::Generic(name, _)
                | ast::TypeRefKind::AppliedMember { name, .. } => Some(name),
                ast::TypeRefKind::Qualified { path, .. } => path.last(),
                _ => None,
            };
            return Some(CallInput::from_arguments(name, base.ty.span.end, arguments));
        }
        let owner = self.construction_body_owner(Body::Delegation(constructor));
        let input = inputs.sources.get(&owner)?;
        let constructors = match constructor {
            Constructor::Class(id) => {
                self.class_secondary_sources(self.class_constructors[id].owner)
            }
            Constructor::Struct(id) => self.structs[self.struct_constructors[id].owner]
                .constructors
                .iter()
                .copied()
                .filter(|id| {
                    matches!(
                        self.struct_constructors[*id].kind,
                        hir::StructConstructorKind::Secondary { .. }
                    )
                })
                .map(Constructor::Struct)
                .collect(),
        };
        let source = constructors
            .into_iter()
            .zip(input.secondary())
            .find(|(id, _)| *id == constructor)?
            .1;
        let (text, target, arguments, actual_span) = match source.delegation.as_ref()? {
            ast::ConstructorDelegation::This {
                target_span,
                arguments,
                span,
            } => ("this", *target_span, arguments, *span),
            ast::ConstructorDelegation::Super {
                target_span,
                arguments,
                span,
            } => ("super", *target_span, arguments, *span),
        };
        if actual_span != span {
            return None;
        }
        let name = ast::Ident {
            text: text.into(),
            span: target,
        };
        Some(CallInput::from_arguments(
            Some(&name),
            target.end,
            arguments,
        ))
    }

    pub(crate) fn observe_constructor_parameters(
        &mut self,
        parameters: &[hir::ConstructorParameter],
    ) {
        if !self.observes_current_body() {
            return;
        }
        let context = self.observe_body_context();
        for parameter in parameters {
            let definition = parameter.definition;
            let occurrence = hir::EditingLocalOccurrence {
                span: definition.span,
                context,
                reference: hir::EditingLocalReference {
                    binding: parameter.binding,
                    name: parameter.name.clone(),
                    definition: hir::SemanticSourceLocation {
                        source: self.source_identity(definition.file as usize).clone(),
                        span: definition.span,
                    },
                },
                value_type: parameter.ty,
                role: hir::EditingLocalRole::Declaration,
            };
            self.editing_local_definitions
                .insert(parameter.binding, occurrence);
        }
    }
}
