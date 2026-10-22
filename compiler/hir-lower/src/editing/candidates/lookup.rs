use super::*;
use crate::imports::lookup::calls::{NamedCallBinding, NamedCallOrigin, NamedCallTarget};
use ast::RecoveredCallTarget as Target;
use hir::ImportedCallableSource;

mod functions;
mod values;

impl Lowerer {
    pub(in crate::editing) fn editing_call_candidates(
        &self,
        call: &ast::RecoveredCall,
        is_cancelled: &impl Fn() -> bool,
    ) -> Result<V<Vec<hir::EditingCallCandidate>>, EditingCancelled> {
        let mut state = self.clone();
        let before = state.diagnostics.len();
        match &call.target {
            Target::Named(name) => state.editing_named_call_candidates(call, name, is_cancelled),
            Target::Member {
                receiver,
                name,
                navigation,
            } => {
                let Some(value) = state
                    .lower_expr(receiver, &mut Vec::new(), None)
                    .filter(|_| state.diagnostics.len() == before)
                else {
                    return Ok(V::Unavailable(U::BodyFailure));
                };
                let value = if *navigation == ast::Navigation::Safe {
                    let Some((_, payload)) =
                        state.prepare_safe_call_receiver(value, call.span, &mut Vec::new())
                    else {
                        return Ok(V::Unavailable(U::BodyFailure));
                    };
                    payload
                } else {
                    value
                };
                state.editing_member_candidates(call, value, &name.text, false, false, is_cancelled)
            }
            Target::Invoke(expression) => {
                let Some(value) = state
                    .lower_expr(expression, &mut Vec::new(), None)
                    .filter(|_| state.diagnostics.len() == before)
                else {
                    return Ok(V::Unavailable(U::BodyFailure));
                };
                state.editing_value_call_candidates(call, value, is_cancelled)
            }
            Target::Super { name, .. } => {
                let Some(receiver) = state.resolve_super_call_receiver(name, call.span) else {
                    return Ok(V::Unavailable(U::BodyFailure));
                };
                state.editing_member_candidates(
                    call,
                    receiver,
                    &name.text,
                    false,
                    true,
                    is_cancelled,
                )
            }
            Target::QualifiedSuper {
                qualifier, name, ..
            } => {
                let Some((receiver, _)) =
                    state.resolve_direct_interface_super(qualifier, call.span)
                else {
                    return Ok(V::Unavailable(U::BodyFailure));
                };
                state.editing_member_candidates(
                    call,
                    receiver,
                    &name.text,
                    false,
                    true,
                    is_cancelled,
                )
            }
            Target::Unavailable(_) => Ok(V::Unavailable(U::IncompleteSyntax)),
        }
    }

    fn editing_named_call_candidates(
        &mut self,
        call: &ast::RecoveredCall,
        name: &ast::Ident,
        is_cancelled: &impl Fn() -> bool,
    ) -> Result<V<Vec<hir::EditingCallCandidate>>, EditingCancelled> {
        if let Some(ty) = self.named_callable_value_type(&name.text) {
            let Some(value) = self.lower_var(name, &mut Vec::new(), Some(ty)) else {
                return Ok(V::Unavailable(U::BodyFailure));
            };
            return self.editing_value_call_candidates(call, value, is_cancelled);
        }
        let mut local = Candidates::default();
        for id in self.local_function_scopes.lookup(&name.text) {
            super::super::receivers::check_cancelled(is_cancelled)?;
            let function = self.local_functions[id].source_function();
            let owner_arguments =
                self.ambient_type_args(self.local_functions[id].owner_type_param_count);
            local.add(self.editing_source_call_candidate(
                call,
                &crate::CallableCandidate::function(function, owner_arguments),
                None,
            ));
        }
        if local.unavailable.is_some() || !local.values.is_empty() {
            return Ok(local.finish());
        }
        if let Some(alias) = self.lexical_type_alias(&name.text) {
            let binding = NamedCallBinding {
                target: NamedCallTarget::Type(crate::namespace::TopLevelTypeTarget::Alias(alias)),
                origin: NamedCallOrigin::Lexical(alias),
            };
            return self.editing_named_nominal_candidates(&binding, call, is_cancelled);
        }
        if let Some(nominal) = self.lexical_nested_nominal_target(&name.text) {
            let candidates =
                self.editing_lexical_nominal_candidates(nominal, call, is_cancelled)?;
            if !matches!(&candidates, V::Known(values) if values.is_empty()) {
                return Ok(candidates);
            }
        }
        let mut implicit_properties = Vec::new();
        for binding in self.implicit_receivers() {
            super::super::receivers::check_cancelled(is_cancelled)?;
            let Some(receiver) = self.lower_receiver_binding(binding, name.span) else {
                return Ok(V::Unavailable(U::BodyFailure));
            };
            let candidates = self.editing_direct_member_candidates(
                call,
                receiver.ty,
                &name.text,
                false,
                false,
                is_cancelled,
            )?;
            if candidates.unavailable.is_some() || !candidates.values.is_empty() {
                return Ok(candidates.finish());
            }
            if let Some(mut property) = self.editing_member_property(name, &receiver) {
                let direct = property.state.editing_direct_value_candidates(
                    call,
                    &property.expression,
                    is_cancelled,
                )?;
                if !matches!(&direct, V::Known(values) if values.is_empty()) {
                    return Ok(direct);
                }
                implicit_properties.push(property);
            }
        }
        let mut pending = Vec::new();
        for layer in self.named_call_layers(&name.text) {
            super::super::receivers::check_cancelled(is_cancelled)?;
            if !self
                .unavailable_named_call_candidates(&layer.unavailable)
                .is_empty()
            {
                return Ok(V::Unavailable(U::SignatureFailure));
            }
            let rank = layer.kind.call_rank();
            let candidates =
                self.editing_named_function_candidates(call, &layer.candidates, is_cancelled)?;
            if candidates.unavailable.is_some() || !candidates.values.is_empty() {
                return Ok(candidates.finish());
            }
            for property in &mut implicit_properties {
                let extensions = property.state.editing_value_extensions_at(
                    call,
                    &property.expression,
                    0,
                    rank,
                    is_cancelled,
                )?;
                if !matches!(&extensions, V::Known(values) if values.is_empty()) {
                    return Ok(extensions);
                }
            }
            match self.editing_named_values(&layer.candidates, name, rank) {
                Ok(Some(mut property)) => {
                    let direct = property.view.state.editing_direct_value_candidates(
                        call,
                        &property.view.expression,
                        is_cancelled,
                    )?;
                    if !matches!(&direct, V::Known(values) if values.is_empty()) {
                        return Ok(direct);
                    }
                    pending.push(property);
                }
                Ok(None) => {}
                Err(reason) => return Ok(V::Unavailable(reason)),
            }
            let mut combined = Candidates::default();
            for property in &mut pending {
                match property.view.state.editing_value_extensions_at(
                    call,
                    &property.view.expression,
                    property.rank,
                    rank,
                    is_cancelled,
                )? {
                    V::Known(values) => combined.values.extend(values),
                    V::Unavailable(reason) => {
                        combined.unavailable.get_or_insert(reason);
                    }
                }
            }
            if combined.unavailable.is_some() || !combined.values.is_empty() {
                return Ok(combined.finish());
            }
            if !layer.suppressed_callables.is_empty() {
                return Ok(V::Unavailable(U::SignatureFailure));
            }
        }
        Ok(V::Known(Vec::new()))
    }
}
