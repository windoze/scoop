//! Required calls expose additional specialization edges after body checking.

use super::*;
use scoop_ast::{Diagnostic, DiagnosticSeverity};
use std::collections::HashSet;
use type_info::CallableBody;

impl Concretizer<'_> {
    pub(super) fn record_specialization_request(&mut self, body: CallableBody) {
        if self.source.member_requirement_implementations.is_empty() {
            return;
        }
        let Some(parent) = self.current_body else {
            return;
        };
        let parents = self.specialization_parents.entry(body).or_default();
        if !parents.contains(&parent) {
            parents.push(parent);
        }
        let Some((template, arguments)) = self.recursion_signature(body) else {
            return;
        };
        if arguments.is_empty() {
            return;
        }
        let mut pending = vec![parent];
        let mut visited = HashSet::new();
        while let Some(ancestor) = pending.pop() {
            if !visited.insert(ancestor) {
                continue;
            }
            if let Some((prior, prior_arguments)) = self.recursion_signature(ancestor)
                && prior == template
                && prior_arguments != arguments
            {
                let message = "polymorphic recursion is not supported: a member-requirement call chain changes a callable's complete type-argument mapping".to_owned();
                self.type_condition_errors.push(match self.type_use_site {
                    Some((file, span)) => Diagnostic::at_file(file, span, message),
                    None => Diagnostic::without_span(DiagnosticSeverity::Error, 0, message),
                });
                return;
            }
            pending.extend(
                self.specialization_parents
                    .get(&ancestor)
                    .into_iter()
                    .flatten()
                    .copied(),
            );
        }
    }

    fn recursion_signature(
        &self,
        body: CallableBody,
    ) -> Option<(scoop_identity::CallableTemplateOwner, &[concrete::TypeId])> {
        if let CallableBody::Function(id) = body {
            let key = &self.function_keys[id.into_raw().into_u32() as usize];
            return key
                .template_owner()
                .map(|owner| (owner, &key.arguments[..]));
        }
        let (owner, _, arguments) = self.metadata_parent_request(body);
        Some((owner, arguments))
    }
}
