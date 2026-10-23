use super::*;
use crate::expr::members::CurrentQualifiedCall;

mod aliases;
mod imported;

impl Lowerer {
    pub(crate) fn editing_qualified_candidates(
        &mut self,
        call: &ast::RecoveredCall,
        receiver: &ast::Expr,
        name: &ast::Ident,
        is_cancelled: &impl Fn() -> bool,
    ) -> Result<Option<CallCandidates>, EditingCancelled> {
        if is_cancelled() {
            return Err(EditingCancelled);
        }
        if let Some(parameter) = self.companion_parameter_qualifier(receiver) {
            let Some(receiver) = self.lower_parameter_companion(parameter, receiver.span()) else {
                return Ok(Some(V::Unavailable(U::BodyFailure)));
            };
            return self
                .editing_member_candidates(call, receiver, &name.text, false, false, is_cancelled)
                .map(Some);
        }
        let alias_variant = match self.resolve_alias_variant(receiver, name) {
            Ok(candidate) => candidate,
            Err(()) => return Ok(Some(V::Unavailable(U::DeclarationFailure))),
        };
        if let Some((alias, index)) = alias_variant {
            return Ok(Some(
                self.editing_alias_variant_candidates(call, &alias, index, name),
            ));
        }
        let alias = match self.nested_constructor_alias(receiver, name) {
            Ok(candidate) => candidate,
            Err(()) => return Ok(Some(V::Unavailable(U::DeclarationFailure))),
        };
        if let Some(alias) = alias {
            let prepared = match self.named_alias_plans(alias, header(call), None) {
                Ok(plans) => plans,
                Err(()) => return Ok(Some(V::Unavailable(U::DeclarationFailure))),
            };
            return self
                .editing_prepared_nominal_candidates(call, prepared, is_cancelled)
                .map(Some);
        }
        let imported = match self.resolve_imported_nominal_qualifier(receiver) {
            Ok(candidate) => candidate,
            Err(()) => return Ok(Some(V::Unavailable(U::DeclarationFailure))),
        };
        if let Some(owner) = imported {
            return self
                .editing_imported_qualified_candidates(call, owner, name, is_cancelled)
                .map(Some);
        }
        self.editing_current_qualified_candidates(call, receiver, name, is_cancelled)
    }

    fn editing_current_qualified_candidates(
        &mut self,
        call: &ast::RecoveredCall,
        receiver: &ast::Expr,
        name: &ast::Ident,
        is_cancelled: &impl Fn() -> bool,
    ) -> Result<Option<CallCandidates>, EditingCancelled> {
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
