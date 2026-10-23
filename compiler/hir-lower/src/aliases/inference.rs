use super::*;
use crate::call_resolution::applicability::{
    DeclarationApplicabilityInput, DeclarationTypeArguments,
};
use crate::call_resolution::candidates::DeclarationSignature;
use crate::call_resolution::constraints::{Constraint, ConstraintOrigin, TypeTerm};
use crate::expr::ResolvedCallTypeArgument;

impl Lowerer {
    /// Resolve a target whose constructor supplies no inference inputs, such as
    /// a unit variant, a pointer, or an alias to a type parameter.
    pub(crate) fn infer_alias_target(
        &mut self,
        alias: &AliasTemplate,
        explicit: &[ResolvedCallTypeArgument],
        expected: Option<hir::TypeId>,
        span: ast::Span,
    ) -> Option<(hir::TypeId, Vec<hir::TypeId>)> {
        let signature: DeclarationSignature = DeclarationSignature {
            availability: None,
            owner_parameters: Vec::new(),
            callable_parameters: Vec::new(),
            value_parameters: Vec::new(),
            return_type: alias.target,
        };
        let mapped = self.alias_constructor_signature(alias, &signature, explicit, span)?;
        let (mut session, environment) =
            self.declaration_applicability_session(DeclarationApplicabilityInput {
                availability: mapped.signature.availability,
                owner_parameters: &[],
                callable_parameters: &mapped.signature.callable_parameters,
                type_arguments: DeclarationTypeArguments::Callable {
                    owner_arguments: &[],
                },
                explicit_arguments: &mapped.explicit,
                bound_receiver: None,
            });
        if let Some(expected) =
            expected.filter(
                |expected| match (&self.types[alias.target], &self.types[*expected]) {
                    (Type::Param(_), _) | (Type::Ptr(_), Type::Ptr(_)) => true,
                    _ if self
                        .nominal_application(alias.target)
                        .zip(self.nominal_application(*expected))
                        .is_some_and(|(left, right)| left.template == right.template) =>
                    {
                        true
                    }
                    _ => false,
                },
            )
        {
            session.push(
                Constraint::Equal(
                    TypeTerm::Type(mapped.signature.return_type),
                    TypeTerm::Rigid(expected),
                ),
                ConstraintOrigin::ExpectedResult,
            );
        }
        match self.solve_constraints(&session) {
            Ok(solution) => {
                let arguments = solution.arguments_for(&session, environment).callable;
                let bindings = mapped
                    .signature
                    .callable_parameters
                    .iter()
                    .map(|parameter| parameter.id)
                    .zip(arguments.iter().copied())
                    .collect::<Vec<_>>();
                Some((
                    self.instantiate_method_ty(mapped.signature.return_type, &bindings),
                    arguments,
                ))
            }
            Err(failure) => {
                let reason = self.render_imported_constraint_failure(
                    &[],
                    &mapped.signature.callable_parameters,
                    &failure,
                );
                self.error(span, format!("typealias `{}`: {reason}", alias.name.text));
                None
            }
        }
    }
}
