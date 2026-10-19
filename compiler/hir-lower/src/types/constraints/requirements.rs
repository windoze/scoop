use super::*;
use std::collections::HashMap;

mod effects;
mod subjects;

impl Lowerer {
    pub(super) fn resolve_member_requirements(
        &mut self,
        parameters: &mut [hir::TypeParamDecl],
        own: &HashMap<String, usize>,
        clause: Option<&ast::WhereClause>,
        allowed: bool,
        target: &str,
    ) {
        for block in clause.into_iter().flat_map(|clause| &clause.requirements) {
            if !allowed || own.is_empty() {
                self.error(
                    block.span,
                    format!(
                        "member requirements are only allowed on generic functions, not {target}"
                    ),
                );
                continue;
            }
            let virtuals = self.requirement_parameters(block);
            for call in &block.calls {
                let Some((index, contract)) =
                    self.resolve_member_requirement(call, &virtuals, parameters, own)
                else {
                    continue;
                };
                let parameter = &mut parameters[index];
                if let Some(previous) = parameter
                    .requirements
                    .iter()
                    .find(|requirement| requirement.contract == contract)
                {
                    let mut error = ast::Diagnostic::at(call.span, "duplicate member requirement");
                    error.file = self.current_file;
                    error.notes.push(ast::DiagnosticNote::at(
                        self.current_file,
                        previous.span,
                        "previous requirement is here",
                    ));
                    self.diagnostics.push(error);
                    continue;
                }
                let id = hir::MemberRequirementId {
                    declaration_parameter: parameter.id,
                    ordinal: parameter
                        .requirements
                        .len()
                        .try_into()
                        .expect("source requirements fit u32"),
                };
                parameter.requirements.push(hir::MemberRequirement {
                    id,
                    contract,
                    span: call.span,
                });
            }
        }
    }

    fn requirement_parameters(
        &mut self,
        block: &ast::MemberRequirements,
    ) -> HashMap<String, TypeId> {
        let mut virtuals = HashMap::new();
        for parameter in &block.parameters {
            if virtuals.contains_key(&parameter.name.text) {
                self.error(
                    parameter.name.span,
                    format!("duplicate virtual parameter `{}`", parameter.name.text),
                );
                continue;
            }
            if let Some(ty) = self.resolve_type_ref(&parameter.ty) {
                virtuals.insert(parameter.name.text.clone(), ty);
            }
        }
        virtuals
    }

    fn resolve_member_requirement(
        &mut self,
        source: &ast::MemberRequirement,
        virtuals: &HashMap<String, TypeId>,
        parameters: &[hir::TypeParamDecl],
        own: &HashMap<String, usize>,
    ) -> Option<(usize, hir::MemberRequirementContract)> {
        let effects = self.member_requirement_effects(source)?;
        let (subject, call, result) = match &source.call {
            ast::RequiredCall::Method {
                receiver,
                companion,
                name,
                type_arguments,
                result,
            } => {
                let subject =
                    self.requirement_receiver(receiver, *companion, virtuals, parameters, own)?;
                let mut arguments = Vec::new();
                for argument in type_arguments {
                    arguments.push(match argument {
                        ast::CallTypeArgument::Explicit(ty) => {
                            hir::RequiredTypeArgument::Explicit(self.resolve_type_ref(ty)?)
                        }
                        ast::CallTypeArgument::Infer { .. } => hir::RequiredTypeArgument::Infer,
                    });
                }
                let call = hir::RequiredCallKind::Method {
                    name: name.text.clone(),
                    type_arguments: arguments,
                };
                (subject, call, self.resolve_type_ref(result)?)
            }
            ast::RequiredCall::Constructor { parameter, result } => {
                let Some(&index) = own.get(&parameter.text) else {
                    self.error(
                        parameter.span,
                        "a construction requirement must name this function's type parameter"
                            .into(),
                    );
                    return None;
                };
                let subject = self.intern_type(Type::Param(parameters[index].id));
                if let Some(result) = result
                    && self.resolve_type_ref(result)? != subject
                {
                    self.error(result.span, "a construction requirement's result must be the constructed type parameter".into());
                    return None;
                }
                if effects.is_suspend {
                    self.error(
                        source.span,
                        "a construction requirement cannot be suspend".into(),
                    );
                    return None;
                }
                (subject, hir::RequiredCallKind::Constructor, subject)
            }
        };
        let parameter = match self.types[subject] {
            Type::Param(id) | Type::Companion(id) => id,
            _ => {
                self.error(source.span, "a requirement receiver must be this function's type parameter or its companion".into());
                return None;
            }
        };
        let Some(index) = own
            .values()
            .copied()
            .find(|&index| parameters[index].id == parameter)
        else {
            self.error(
                source.span,
                "a requirement cannot constrain an outer or host type parameter".into(),
            );
            return None;
        };
        let mut arguments = Vec::new();
        for argument in &source.arguments {
            let ty = match &argument.value {
                ast::RequirementArgumentValue::Inline { ty, .. } => self.resolve_type_ref(ty)?,
                ast::RequirementArgumentValue::Parameter(name) => {
                    let Some(&ty) = virtuals.get(&name.text) else {
                        self.error(
                            name.span,
                            format!("unknown virtual parameter `{}`", name.text),
                        );
                        return None;
                    };
                    ty
                }
            };
            arguments.push(hir::RequiredCallArgument {
                name: argument.name.as_ref().map(|name| name.text.clone()),
                spread: matches!(argument.spread, ast::SpreadSyntax::Spread(_)),
                ty,
            });
        }
        Some((
            index,
            hir::MemberRequirementContract {
                subject,
                call,
                arguments,
                result,
                effects,
            },
        ))
    }
}
