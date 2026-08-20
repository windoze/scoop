//! Only substitution can merge keys that passed the declaration check.

use super::*;
use scoop_ast::{Diagnostic, DiagnosticSeverity};

impl Concretizer<'_> {
    pub(super) fn check_loaded_contexts(
        &mut self,
        contracts: &[export::LoadedContextContract],
        substitution: &[concrete::TypeId],
    ) {
        for contract in contracts {
            self.check_context_instantiation(
                export::ContextRequirementOwner::Imported(contract.declaration),
                &contract.name,
                &contract.parameters,
                substitution,
            );
        }
    }

    pub(super) fn check_context_instantiation(
        &mut self,
        declaration: export::ContextRequirementOwner,
        name: &str,
        parameters: &[export::TypeId],
        substitution: &[concrete::TypeId],
    ) {
        if substitution.is_empty() || parameters.len() < 2 {
            return;
        }
        let keys = parameters
            .iter()
            .map(|ty| self.lower_type(*ty, substitution))
            .collect::<Vec<_>>();
        if !self
            .checked_context_requirements
            .insert((declaration, keys.clone()))
        {
            return;
        }
        let mut positions = HashMap::new();
        for (index, key) in keys.into_iter().enumerate() {
            if let Some(previous) = positions.insert(key, index) {
                let message = format!(
                    "duplicate context key after specialization of `{name}`: parameters {} and {} have the same exact type",
                    previous + 1,
                    index + 1,
                );
                self.type_condition_errors.push(match self.type_use_site {
                    Some((file, span)) => Diagnostic::at_file(file, span, message),
                    None => Diagnostic::without_span(DiagnosticSeverity::Error, 0, message),
                });
                return;
            }
        }
    }

    pub(super) fn check_source_method_context(
        &mut self,
        function: export::FunctionId,
        substitution: &[concrete::TypeId],
    ) {
        let source = &self.source.functions[function];
        if source.method_type_param_count() != 0 {
            return;
        }
        self.check_context_instantiation(
            export::ContextRequirementOwner::Source(function),
            &source.name,
            &source
                .context_parameters
                .iter()
                .map(|p| p.ty)
                .collect::<Vec<_>>(),
            substitution,
        );
    }
}
