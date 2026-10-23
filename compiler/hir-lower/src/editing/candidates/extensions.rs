use super::*;
use crate::expr::RequiredCallableModifiers;
use crate::imports::lookup::LookupLayer;
use crate::imports::lookup::calls::ExtensionCallTarget;

impl Lowerer {
    pub(super) fn editing_extension_layer(
        &mut self,
        call: &ast::RecoveredCall,
        types: &[hir::TypeId],
        layer: &LookupLayer<ExtensionCallTarget>,
        required: RequiredCallableModifiers,
        is_cancelled: &impl Fn() -> bool,
    ) -> Result<Candidates, EditingCancelled> {
        let mut result = Candidates::default();
        if !self
            .unavailable_extension_candidates(&layer.unavailable, required)
            .is_empty()
        {
            result.unavailable = Some(U::SignatureFailure);
            return Ok(result);
        }
        for target in &layer.candidates {
            super::super::receivers::check_cancelled(is_cancelled)?;
            if !self.extension_call_target_matches_required(target, required) {
                continue;
            }
            for &ty in types {
                let candidate = match target {
                    ExtensionCallTarget::Current(function) => self.editing_source_call_candidate(
                        call,
                        &crate::CallableCandidate::function(*function, Vec::new()),
                        Some(ty),
                    ),
                    ExtensionCallTarget::Dependency(binding) => {
                        let declaration = self.dependencies.as_ref().and_then(|dependencies| {
                            dependencies
                                .callable_candidate(binding)
                                .ok()
                                .and_then(|candidate| {
                                    dependencies
                                        .callable_declaration(candidate.interface().declaration())
                                        .ok()
                                })
                        });
                        match declaration {
                            Some(declaration) => {
                                self.editing_imported_call_candidate(call, &declaration, Some(ty))
                            }
                            None => Err(U::DeclarationFailure),
                        }
                    }
                };
                let available = matches!(&candidate, Ok(Some(_)));
                result.add(candidate);
                if available {
                    break;
                }
            }
        }
        if result.values.is_empty()
            && layer
                .suppressed_callables
                .iter()
                .any(|&function| self.function_matches_required_modifiers(function, required))
        {
            result.unavailable = Some(U::SignatureFailure);
        }
        Ok(result)
    }

    pub(super) fn editing_value_extensions_at(
        &mut self,
        call: &ast::RecoveredCall,
        value: &hir::Expr,
        property_rank: usize,
        rank: usize,
        is_cancelled: &impl Fn() -> bool,
    ) -> Result<CallCandidates, EditingCancelled> {
        if matches!(
            self.types[value.ty],
            hir::Type::Function(_) | hir::Type::FunPtr(_)
        ) {
            return Ok(V::Known(Vec::new()));
        }
        let types = self
            .smart_cast_receiver_views(value)
            .iter()
            .map(|view| view.ty)
            .collect::<Vec<_>>();
        let required = RequiredCallableModifiers {
            operator: Some(hir::OperatorKind::Invoke),
            ..Default::default()
        };
        let mut result = Candidates::default();
        let mut layers = self.named_executable_extension_operator_layers(hir::OperatorKind::Invoke);
        Self::retain_highest_rank_origins(&mut layers);
        for layer in layers {
            if property_rank.max(layer.kind.call_rank()) != rank {
                continue;
            }
            let candidates =
                self.editing_extension_layer(call, &types, &layer, required, is_cancelled)?;
            result.values.extend(candidates.values);
            result.unavailable = result.unavailable.or(candidates.unavailable);
        }
        Ok(result.finish())
    }
}
