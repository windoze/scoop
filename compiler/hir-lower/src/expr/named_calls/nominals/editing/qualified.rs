use super::*;
use crate::expr::members::CurrentQualifiedCall;

impl Lowerer {
    pub(crate) fn editing_current_qualified_candidates(
        &mut self,
        call: &ast::RecoveredCall,
        receiver: &ast::Expr,
        name: &ast::Ident,
        is_cancelled: &impl Fn() -> bool,
    ) -> Result<Option<V<Vec<hir::EditingCallCandidate>>>, EditingCancelled> {
        if is_cancelled() {
            return Err(EditingCancelled);
        }
        let target =
            match self.resolve_current_qualified_call(receiver, name, &call.type_args, None) {
                Ok(Some(target)) => target,
                Ok(None) => return Ok(None),
                Err(()) => return Ok(Some(V::Unavailable(U::BodyFailure))),
            };
        let result = match target {
            CurrentQualifiedCall::Struct { id, .. } => self.editing_lexical_nominal_candidates(
                crate::NominalTarget::Struct(id),
                call,
                is_cancelled,
            )?,
            CurrentQualifiedCall::Class(id) => self.editing_lexical_nominal_candidates(
                crate::NominalTarget::Class(id),
                call,
                is_cancelled,
            )?,
            CurrentQualifiedCall::Variant { target, expected } => {
                let mut result = Candidates::default();
                let source = NominalConstructorSource::Variant(target);
                if self.resolved_variant_style(target) != VariantStyle::Unit
                    && self.constructor_is_accessible(source)
                {
                    let view = self.nominal_constructor_view(source, call.span);
                    result.add(self.editing_nominal_signature(
                        call,
                        view.signature,
                        view.argument_mode,
                        expected,
                        None,
                    ));
                }
                result.finish()
            }
            CurrentQualifiedCall::Singleton(receiver) => self.editing_member_candidates(
                call,
                receiver,
                &name.text,
                false,
                false,
                is_cancelled,
            )?,
        };
        Ok(Some(result))
    }
}
