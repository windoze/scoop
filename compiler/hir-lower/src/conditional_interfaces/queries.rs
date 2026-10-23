use super::*;
use crate::call_resolution::applicability::{
    DeclarationApplicabilityInput, DeclarationTypeArguments,
};

impl Lowerer {
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
