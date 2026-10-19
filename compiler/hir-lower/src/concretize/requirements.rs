use super::*;

#[derive(Clone)]
pub(super) struct ResolvedRequirement {
    implementation: export::MemberRequirementImplementationId,
    arguments: Vec<concrete::TypeId>,
}
pub(super) type RequirementEnvironment = HashMap<export::MemberRequirementId, ResolvedRequirement>;

impl Concretizer<'_> {
    fn resolve_requirement(
        &mut self,
        binding: &export::MemberRequirementBindingValue,
        substitution: &[concrete::TypeId],
    ) -> ResolvedRequirement {
        match binding {
            export::MemberRequirementBindingValue::Forwarded(id) => self
                .requirement_environment
                .get(id)
                .expect("a required call has a checked enclosing application binding")
                .clone(),
            export::MemberRequirementBindingValue::Implementation {
                implementation,
                type_arguments,
            } => ResolvedRequirement {
                implementation: *implementation,
                arguments: type_arguments
                    .iter()
                    .map(|ty| self.lower_type(*ty, substitution))
                    .collect(),
            },
        }
    }

    pub(super) fn with_requirement_bindings<T>(
        &mut self,
        bindings: &[export::MemberRequirementBinding],
        substitution: &[concrete::TypeId],
        action: impl FnOnce(&mut Self) -> T,
    ) -> T {
        if bindings.is_empty() {
            return action(self);
        }
        let resolved = bindings
            .iter()
            .map(|binding| {
                (
                    binding.requirement,
                    self.resolve_requirement(&binding.value, substitution),
                )
            })
            .collect::<Vec<_>>();
        let saved = self.requirement_environment.clone();
        self.requirement_environment.extend(resolved);
        let result = action(self);
        self.requirement_environment = saved;
        result
    }

    pub(super) fn lower_requirement_call(
        &mut self,
        call: &export::MemberRequirementCall,
        substitution: &[concrete::TypeId],
        locals: &[concrete::LocalId],
        storage: &mut Arena<concrete::Local>,
        out: &mut Vec<concrete::Statement>,
    ) {
        let binding = self.resolve_requirement(&call.binding, substitution);
        let plan = &self.source.member_requirement_implementations[binding.implementation];
        let inputs = call
            .arguments
            .iter()
            .map(|value| self.lower_expr(value, substitution, locals))
            .collect::<Vec<_>>();
        let result_local = self.lower_local(call.result, locals);
        let origin = self.concrete_expression_origin(call.origin);
        let saved_evaluation = self.requirement_evaluation.replace(origin.evaluation);
        let (plan_locals, _) = self.lower_locals(&plan.body.locals, &binding.arguments);
        let mut mapping = Vec::new();
        use scoop_identity::{
            LocalValueSelector, StructuralDefinitionPath, StructuralDefinitionSiteRole as Role,
            StructuralPathSegment,
        };
        let result_path = match &storage[result_local].selector {
            LocalValueSelector::Synthetic { path, .. } => path.clone(),
            _ => unreachable!("a required-call result is a compiler temporary"),
        };
        for (id, mut local) in plan_locals {
            let path =
                StructuralDefinitionPath::from_first(
                    result_path.segments()[0],
                    result_path.segments()[1..].iter().copied().chain([
                        StructuralPathSegment::new(Role::SyntheticValue, id.into_raw().into_u32()),
                    ]),
                );
            local.selector = match local.definition {
                export::LocalValueDefinitionSite::Synthetic => LocalValueSelector::Synthetic {
                    path,
                    role: scoop_identity::SyntheticLocalRole::Temporary,
                },
                export::LocalValueDefinitionSite::Source(_) => {
                    LocalValueSelector::LocalDeclaration { path }
                }
            };
            mapping.push(storage.alloc(local));
        }
        assert_eq!(plan.inputs.len(), inputs.len());
        for (&input, value) in plan.inputs.iter().zip(inputs) {
            out.push(concrete::Statement {
                span: origin.evaluation.span,
                kind: concrete::StatementKind::ValDecl {
                    pattern: concrete::Pattern::Binding {
                        local: mapping[input.into_raw().into_u32() as usize],
                    },
                    init: value,
                },
            });
        }
        out.extend(self.lower_statement_region(
            &plan.body.statements,
            &binding.arguments,
            &mapping,
            storage,
        ));
        let value = self.lower_expr(&plan.result, &binding.arguments, &mapping);
        out.push(concrete::Statement {
            span: origin.evaluation.span,
            kind: concrete::StatementKind::ValDecl {
                pattern: concrete::Pattern::Binding {
                    local: result_local,
                },
                init: value,
            },
        });
        self.requirement_evaluation = saved_evaluation;
    }
}
