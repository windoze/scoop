use super::*;
use hir::ImportedCallableSource;

impl Lowerer {
    pub(super) fn check_override_condition_coverage(
        &mut self,
        owner: Owner,
        function: FunctionId,
        local: &[(FunctionId, Vec<TypeId>)],
        imported: &[super::super::imported::ImportedInheritedMethod],
        span: ast::Span,
    ) {
        let Some(condition) = self.functions[function]
            .method
            .and_then(|method| method.availability)
        else {
            return;
        };
        let required = hir::ConditionalInterfaceApplication {
            declaration: condition,
            owner: self.nominal_identity(owner).declaration_id(),
            arguments: self.owner_type_args(owner),
        };
        // Coverage uses the derived type's domain, not the overriding body's
        // stronger assumptions. The latter must not prove its own coverage.
        let parameters = self.owner_type_params(owner);
        let saved = std::mem::replace(&mut self.type_params_in_scope, parameters);
        let mut inherited = Vec::new();
        for (function, arguments) in local {
            let Some(owner @ Owner::Class(_)) = self.function_owner.get(function).copied() else {
                continue;
            };
            inherited.push(self.functions[*function].method.unwrap().availability.map(
                |condition| hir::ConditionalInterfaceApplication {
                    declaration: condition,
                    owner: self.nominal_identity(owner).declaration_id(),
                    arguments: arguments.clone(),
                },
            ));
        }
        for method in imported {
            inherited
                .push(self.imported_member_condition(method.declaration.interface(), method.owner));
        }
        for condition in inherited {
            let covered = match condition {
                Some(condition) if self.conditional_application_is_impossible(&condition) => true,
                Some(condition) => self
                    .with_conditional_interface_assumptions(&condition, |state| {
                        state.conditional_application_holds(&required)
                    }),
                None => self.conditional_application_holds(&required),
            };
            if !covered {
                self.error(span, format!(
                    "condition of override `{}` must cover the entire inherited method condition",
                    self.source_function_name(function),
                ));
                break;
            }
        }
        self.type_params_in_scope = saved;
    }
}
