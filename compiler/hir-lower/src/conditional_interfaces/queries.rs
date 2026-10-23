use super::*;
use crate::call_resolution::applicability::{
    DeclarationApplicabilityInput, DeclarationTypeArguments,
};

impl Lowerer {
    pub(crate) fn local_member_condition(
        &self,
        function: hir::FunctionId,
        arguments: &[TypeId],
    ) -> Option<hir::ConditionalInterfaceApplication> {
        let declaration = self.functions[function].method?.availability?;
        let owner = self.function_owner[&function];
        Some(hir::ConditionalInterfaceApplication {
            declaration,
            owner: self.nominal_identity(owner).declaration_id(),
            arguments: arguments.to_vec(),
        })
    }

    pub(crate) fn imported_member_condition(
        &mut self,
        declaration: &hir::CallableDeclarationRecordV1,
        receiver: TypeId,
    ) -> Option<hir::ConditionalInterfaceApplication> {
        let condition = declaration.availability()?;
        let hir::PublicDeclarationOwnerV1::Nominal(owner) = declaration.owner() else {
            unreachable!("a conditional member has a nominal owner")
        };
        let application = self
            .imported_member_owner_type(receiver, owner)
            .expect("an inherited method retains its original owner application");
        Some(hir::ConditionalInterfaceApplication {
            declaration: condition,
            owner,
            arguments: self.imported_owner_arguments(application).to_vec(),
        })
    }

    pub(crate) fn declaration_conditional_interfaces(
        &self,
        owner: hir::SourceNominalId,
    ) -> &[hir::ConditionalInterfaceDeclaration] {
        if let Some(owner) = self.nominal_owners.get(&owner) {
            return self.owner_conditional_interfaces(*owner);
        }
        if let Some(loaded) = self.loaded_class_definitions.get(&owner) {
            &loaded.definition.conditional_interfaces
        } else if let Some(loaded) = self.loaded_struct_definitions.get(&owner) {
            &loaded.definition.conditional_interfaces
        } else if let Some(loaded) = self.loaded_enum_definitions.get(&owner) {
            &loaded.definition.conditional_interfaces
        } else {
            &[]
        }
    }

    pub(crate) fn conditional_application_holds(
        &mut self,
        application: &hir::ConditionalInterfaceApplication,
    ) -> bool {
        let declaration = self
            .declaration_conditional_interfaces(application.owner)
            .iter()
            .find(|declaration| declaration.id() == application.declaration)
            .expect("conditional applications retain their declaration")
            .clone();
        !self
            .applicable_conditional_interfaces(&[declaration], &application.arguments)
            .is_empty()
    }

    pub(crate) fn conditional_application_is_impossible(
        &mut self,
        application: &hir::ConditionalInterfaceApplication,
    ) -> bool {
        let parameters = self
            .declaration_conditional_interfaces(application.owner)
            .iter()
            .find(|declaration| declaration.id() == application.declaration)
            .expect("conditional applications retain their declaration")
            .parameters
            .clone();
        // A bound on one parameter may mention another parameter's companion.
        // Reject missing projections before substituting any of those bounds.
        if parameters
            .iter()
            .zip(&application.arguments)
            .any(|(parameter, &argument)| {
                parameter.companion.is_some()
                    && !matches!(self.types[argument], Type::Param(_) | Type::Companion(_))
                    && self.public_companion_declaration(argument).is_none()
            })
        {
            return true;
        }
        for (parameter, &argument) in parameters.iter().zip(&application.arguments) {
            if self.conditional_bounds_are_impossible(
                argument,
                &parameter.bounds,
                &application.arguments,
            ) {
                return true;
            }
            if let Some(companion) = &parameter.companion {
                match self.projected_companion_type(argument) {
                    Ok(subject)
                        if self.conditional_bounds_are_impossible(
                            subject,
                            &companion.bounds,
                            &application.arguments,
                        ) =>
                    {
                        return true;
                    }
                    Err(_)
                        if !matches!(self.types[argument], Type::Param(_) | Type::Companion(_)) =>
                    {
                        return true;
                    }
                    _ => {}
                }
            }
        }
        false
    }

    fn conditional_bounds_are_impossible(
        &mut self,
        subject: TypeId,
        bounds: &hir::TypeParamBounds,
        arguments: &[TypeId],
    ) -> bool {
        match bounds {
            hir::TypeParamBounds::Unconstrained => false,
            hir::TypeParamBounds::Value { .. } => {
                self.type_satisfies_kind(subject, hir::TypeParamKind::Ref)
            }
            hir::TypeParamBounds::Ref { .. } => {
                self.type_satisfies_kind(subject, hir::TypeParamKind::Value)
            }
            hir::TypeParamBounds::Nominal(bounds) => {
                bounds.in_source_order().into_iter().any(|bound| {
                    let required = self.instantiate_ty(bound.ty(), arguments);
                    match bound {
                        hir::NominalBoundRef::Interface(_) => {
                            self.excludes_interface(subject, required)
                                == crate::types::InterfaceExclusion::Satisfied
                        }
                        hir::NominalBoundRef::Class(_) => {
                            self.conditional_class_bound_is_impossible(subject, required)
                        }
                    }
                })
            }
        }
    }

    fn conditional_class_bound_is_impossible(&mut self, subject: TypeId, required: TypeId) -> bool {
        if !self.type_contains_param(subject) && !self.type_contains_param(required) {
            return !self.is_subtype(subject, required);
        }
        let mut known = self
            .parameter_nominal_bounds(subject)
            .into_iter()
            .filter_map(|bound| {
                matches!(bound, hir::NominalBoundRef::Class(_)).then_some(bound.ty())
            })
            .collect::<Vec<_>>();
        known.extend(
            self.assumed_conditional_parents(subject)
                .into_iter()
                .filter(|&bound| matches!(self.types[bound], Type::Class(_))),
        );
        known
            .into_iter()
            .any(|bound| self.closed_class_bounds_are_disjoint(bound, required))
    }

    pub(super) fn closed_class_bounds_are_disjoint(&mut self, left: TypeId, right: TypeId) -> bool {
        !self.type_contains_param(left)
            && !self.type_contains_param(right)
            && !self.is_subtype(left, right)
            && !self.is_subtype(right, left)
    }

    /// Owner arguments are fixed before checking a block. This uses the same
    /// bound solver as calls, without inferring a different host application.
    pub(crate) fn conditional_parameters_hold(
        &mut self,
        parameters: &[hir::TypeParamDecl],
        arguments: &[TypeId],
    ) -> bool {
        let (session, _) = self.declaration_applicability_session(DeclarationApplicabilityInput {
            owner_parameters: parameters,
            callable_parameters: &[],
            type_arguments: DeclarationTypeArguments::Callable {
                owner_arguments: arguments,
            },
            explicit_arguments: &[],
            bound_receiver: None,
        });
        self.solve_constraints(&session).is_ok()
    }

    pub(crate) fn applicable_conditional_interfaces(
        &mut self,
        declarations: &[hir::ConditionalInterfaceDeclaration],
        arguments: &[TypeId],
    ) -> Vec<TypeId> {
        let mut interfaces = Vec::new();
        for declaration in declarations {
            let key = (declaration.id(), arguments.to_vec());
            if self.conditional_query_repeats(declaration, arguments) {
                continue;
            }
            self.conditional_interfaces.active_queries.push(key);
            let holds = self.conditional_parameters_hold(&declaration.parameters, arguments);
            self.conditional_interfaces.active_queries.pop();
            if holds {
                interfaces.push(self.instantiate_ty(declaration.interface, arguments));
            }
        }
        interfaces
    }
}
