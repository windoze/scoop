use super::*;
use crate::call_resolution::applicability::{
    DeclarationApplicabilityInput, DeclarationTypeArguments,
};

impl Lowerer {
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
                            !self.type_contains_param(subject)
                                && !self.type_contains_param(required)
                                && !self.is_subtype(subject, required)
                        }
                    }
                })
            }
        }
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
