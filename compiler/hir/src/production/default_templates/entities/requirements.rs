use super::*;
use crate::{MemberRequirementBindingValue as Binding, MemberRequirementOwner as Owner};

impl DefaultEntityProjector<'_> {
    pub(in crate::production::default_templates) fn requirement_ref(
        &self,
        id: crate::MemberRequirementId,
    ) -> Result<crate::DefaultMemberRequirementRefV1, DefaultEntityProjectionError> {
        let owner = self.export.member_requirement_owners.get(&id).ok_or(
            DefaultEntityProjectionError::MissingIdentity {
                kind: "member requirement owner",
                index: id.ordinal,
            },
        )?;
        let (declaration, parameters, binders) = match *owner {
            Owner::Source(function) => {
                let source = &self.export.functions[function];
                (
                    self.callable_declaration(function)?,
                    source
                        .type_params()
                        .into_iter()
                        .cloned()
                        .collect::<Vec<_>>(),
                    self.signatures
                        .function_binders(source)
                        .map_err(DefaultEntityProjectionError::Type)?,
                )
            }
            Owner::Imported(template) => {
                let template = &self.export.imported_generic_templates[template];
                let parameters = template.type_parameters.declarations().to_vec();
                let binders = parameters
                    .iter()
                    .enumerate()
                    .map(|(index, parameter)| HirSignatureBinder {
                        parameter: parameter.id,
                        depth: 0,
                        index: index as u32,
                    })
                    .collect();
                (template.declaration.body_owner(), parameters, binders)
            }
        };
        let parameter = parameters
            .iter()
            .position(|parameter| parameter.id == id.declaration_parameter)
            .expect("requirement owner retains its original parameter");
        let declaration_requirement = &parameters[parameter].requirements;
        let requirement = if matches!(owner, Owner::Imported(_)) {
            id.ordinal
        } else {
            let mut keys = declaration_requirement
                .iter()
                .map(|requirement| {
                    Ok((
                        requirement.contract.key(|ty| self.type_key(ty, &binders))?,
                        requirement.id,
                    ))
                })
                .collect::<Result<Vec<_>, DefaultEntityProjectionError>>()?;
            keys.sort_by(|a, b| a.0.cmp(&b.0));
            keys.iter()
                .position(|(_, candidate)| *candidate == id)
                .expect("original requirement is present") as u32
        };
        Ok(crate::DefaultMemberRequirementRefV1 {
            declaration,
            parameter: parameter as u32,
            requirement,
        })
    }

    pub(in crate::production::default_templates) fn requirement_bindings(
        &self,
        bindings: &[crate::MemberRequirementBinding],
        binders: &[HirSignatureBinder],
    ) -> Result<Vec<crate::DefaultRequirementBindingV1>, DefaultEntityProjectionError> {
        bindings
            .iter()
            .map(|binding| {
                Ok(crate::DefaultRequirementBindingV1 {
                    requirement: self.requirement_ref(binding.requirement)?,
                    value: self.requirement_binding(&binding.value, binders)?,
                })
            })
            .collect()
    }

    pub(in crate::production::default_templates) fn requirement_binding(
        &self,
        binding: &Binding,
        binders: &[HirSignatureBinder],
    ) -> Result<crate::DefaultRequirementBindingValueV1, DefaultEntityProjectionError> {
        Ok(match binding {
            Binding::Forwarded(id) => {
                crate::DefaultRequirementBindingValueV1::Forwarded(self.requirement_ref(*id)?)
            }
            Binding::Implementation {
                implementation,
                type_arguments,
            } => {
                let implementation =
                    &self.export.member_requirement_implementations[*implementation];
                let recipe_binders = type_arguments
                    .iter()
                    .enumerate()
                    .filter_map(|(index, ty)| match self.export.types[*ty] {
                        crate::Type::Param(parameter) => Some(HirSignatureBinder {
                            parameter,
                            depth: 0,
                            index: index as u32,
                        }),
                        _ => None,
                    })
                    .collect::<Vec<_>>();
                let (locals, table) = super::super::locals::TemplateLocalProjection::project(
                    self,
                    &implementation.body.locals,
                    &recipe_binders,
                )
                .map_err(|error| {
                    DefaultEntityProjectionError::RequirementLocals(Box::new(error))
                })?;
                let inputs = implementation
                    .inputs
                    .iter()
                    .map(|id| {
                        let selector = &implementation.body.locals[*id].selector;
                        table
                            .records()
                            .iter()
                            .position(|record| record.selector() == selector)
                            .expect("input is in the canonical local table")
                            as u32
                    })
                    .collect();
                let body = super::super::body::project_fragment(
                    self,
                    &locals,
                    &recipe_binders,
                    implementation.result.origin.definition(),
                    &implementation.body.statements,
                    std::slice::from_ref(&implementation.result),
                    table,
                )
                .map_err(|error| DefaultEntityProjectionError::RequirementBody(Box::new(error)))?;
                crate::DefaultRequirementBindingValueV1::Implementation {
                    body,
                    inputs,
                    type_arguments: type_arguments
                        .iter()
                        .map(|ty| self.type_key(*ty, binders))
                        .collect::<Result<_, _>>()?,
                }
            }
        })
    }
}
