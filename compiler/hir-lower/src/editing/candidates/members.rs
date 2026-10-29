use super::*;
use crate::expr::RequiredCallableModifiers;
use hir::ImportedCallableSource;

impl Lowerer {
    pub(super) fn editing_member_call_candidates(
        &mut self,
        call: &ast::RecoveredCall,
        receiver: hir::Expr,
        name: &str,
        operator: bool,
        is_cancelled: &impl Fn() -> bool,
    ) -> Result<CallCandidates, EditingCancelled> {
        self.editing_member_candidates(call, receiver, name, operator, false, is_cancelled)
    }

    pub(crate) fn editing_member_candidates(
        &mut self,
        call: &ast::RecoveredCall,
        receiver: hir::Expr,
        name: &str,
        operator: bool,
        direct: bool,
        is_cancelled: &impl Fn() -> bool,
    ) -> Result<CallCandidates, EditingCancelled> {
        if name == "invoke" && matches!(self.types[receiver.ty], hir::Type::FunPtr(_)) {
            return Ok(V::Known(Vec::new()));
        }
        if name == "invoke"
            && !operator
            && matches!(self.types[receiver.ty], hir::Type::Function(_))
        {
            return self.editing_value_call_candidates(call, receiver, is_cancelled);
        }
        let types = self
            .smart_cast_receiver_views(&receiver)
            .into_iter()
            .map(|view| view.ty)
            .collect::<Vec<_>>();
        let mut candidates = Candidates::default();
        for &ty in &types {
            let member = self.editing_direct_member_candidates(
                call,
                ty,
                name,
                operator,
                direct,
                is_cancelled,
            )?;
            candidates.values.extend(member.values);
            candidates.unavailable = candidates.unavailable.or(member.unavailable);
        }
        if candidates.unavailable.is_some() || !candidates.values.is_empty() || direct {
            return Ok(candidates.finish());
        }
        let mut property = if operator {
            None
        } else {
            call_name(call).and_then(|name| self.editing_member_property(name, &receiver))
        };
        if let Some(property) = &mut property {
            let direct = property.state.editing_direct_value_candidates(
                call,
                &property.expression,
                is_cancelled,
            )?;
            if !matches!(&direct, V::Known(values) if values.is_empty()) {
                return Ok(direct);
            }
        }
        if !operator {
            let values = self.editing_receiver_function_candidates(call, &receiver);
            if !matches!(&values, V::Known(values) if values.is_empty()) {
                return Ok(values);
            }
        }
        let required = RequiredCallableModifiers {
            operator: operator.then_some(hir::OperatorKind::Invoke),
            ..Default::default()
        };
        let mut layers = if operator {
            self.named_executable_extension_operator_layers(hir::OperatorKind::Invoke)
        } else {
            self.named_executable_extension_call_layers(name)
        };
        Self::retain_highest_rank_origins(&mut layers);
        let mut pending = Vec::new();
        for layer in layers {
            super::super::receivers::check_cancelled(is_cancelled)?;
            let rank = layer.kind.call_rank();
            let candidates =
                self.editing_extension_layer(call, &types, &layer, required, is_cancelled)?;
            if candidates.unavailable.is_some() || !candidates.values.is_empty() {
                return Ok(candidates.finish());
            }
            if let Some(property) = &mut property {
                let extension = property.state.editing_value_extensions_at(
                    call,
                    &property.expression,
                    crate::imports::lookup::calls::ImportLookupRank::ImplicitReceiver,
                    rank,
                    is_cancelled,
                )?;
                if !matches!(&extension, V::Known(values) if values.is_empty()) {
                    return Ok(extension);
                }
            }
            if !operator && let Some(name) = call_name(call) {
                match self.editing_extension_property_value(name, &receiver, rank) {
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
            }
        }
        Ok(V::Known(Vec::new()))
    }

    pub(super) fn editing_direct_member_candidates(
        &mut self,
        call: &ast::RecoveredCall,
        receiver: hir::TypeId,
        name: &str,
        operator: bool,
        direct: bool,
        is_cancelled: &impl Fn() -> bool,
    ) -> Result<Candidates, EditingCancelled> {
        let mut result = Candidates::default();
        let required = RequiredCallableModifiers {
            operator: operator.then_some(hir::OperatorKind::Invoke),
            ..Default::default()
        };
        let methods = if direct && matches!(self.types[receiver], hir::Type::Class(_)) {
            self.super_methods_by_name(receiver, name)
        } else {
            self.methods_by_name(receiver, name)
        };
        for method in methods {
            super::super::receivers::check_cancelled(is_cancelled)?;
            if !self.function_matches_required_modifiers(method.function, required) {
                continue;
            }
            if direct
                && self.functions[method.function]
                    .method
                    .is_some_and(|method| method.modifier == hir::MethodModifier::Abstract)
            {
                continue;
            }
            let signature = &self.signatures[&method.function];
            if matches!(self.types[receiver], hir::Type::Interface(_))
                && signature.type_params.len() != signature.owner_type_param_count
            {
                continue;
            }
            result.add(self.editing_source_call_candidate(call, &method, Some(receiver)));
        }
        if self
            .resolve_imported_member_receiver_type(receiver)
            .is_err()
        {
            result.unavailable = Some(U::DeclarationFailure);
            return Ok(result);
        }
        let lookup = if operator {
            hir::ImportedMemberLookup::Operator(hir::CallableOperatorRoleV1::Language(
                crate::imports::lookup::calls::wire_operator(hir::OperatorKind::Invoke),
            ))
        } else {
            hir::ImportedMemberLookup::Name(name)
        };
        let Ok(imported) = self.imported_member_candidates(receiver, lookup) else {
            result.unavailable = Some(U::DeclarationFailure);
            return Ok(result);
        };
        for candidate in imported {
            super::super::receivers::check_cancelled(is_cancelled)?;
            if matches!(self.types[receiver], hir::Type::Interface(_))
                && !candidate.interface().type_parameters().is_empty()
            {
                continue;
            }
            if direct && candidate.interface().modality() == hir::CallableModalityV1::Abstract {
                continue;
            }
            let declaration = self.dependencies.as_ref().and_then(|dependencies| {
                dependencies
                    .callable_declaration(candidate.interface().declaration())
                    .ok()
            });
            result.add(match declaration {
                Some(declaration) => {
                    self.editing_imported_call_candidate(call, &declaration, Some(receiver))
                }
                None => Err(U::DeclarationFailure),
            });
        }
        Ok(result)
    }
}
