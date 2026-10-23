//! Resolve checked implementation conditions before concrete interface tables.

use super::*;

mod relations;

impl Concretizer<'_> {
    pub(super) fn imported_virtual_method_applies(
        &mut self,
        callable: export::ImportedDispatchCallable,
        substitution: &[concrete::TypeId],
    ) -> bool {
        let export::ImportedDispatchCallable::Template(application) = callable else {
            return true;
        };
        let application = &self.source.imported_generic_applications[application];
        let template = &self.source.imported_generic_templates[application.template];
        let arguments = application
            .arguments
            .substitution(
                &self.source.types,
                &self.source.enum_applications,
                &self.source.struct_applications,
                &self.source.class_applications,
                &self.source.interface_applications,
            )
            .into_iter()
            .map(|argument| self.lower_type(argument, substitution))
            .collect::<Vec<_>>();
        match template.availability {
            Some(guard) => {
                let condition = self.source.conditional_guard_application(guard);
                self.concrete_implementation_applies(&condition, &arguments)
            }
            None => self.concrete_conditional_parameters_hold(
                template.type_parameters.declarations(),
                &arguments,
            ),
        }
    }

    pub(super) fn concrete_direct_interfaces(
        &mut self,
        declarations: &[export::ConditionalInterfaceDeclaration],
        ordinary: &[export::TypeId],
        substitution: &[concrete::TypeId],
    ) -> Vec<concrete::TypeId> {
        let mut result = ordinary
            .iter()
            .map(|&interface| self.lower_type(interface, substitution))
            .collect::<Vec<_>>();
        for declaration in declarations {
            if self.concrete_interface_condition_holds(declaration, substitution) {
                let interface = self.lower_type(declaration.interface, substitution);
                if !result.contains(&interface) {
                    result.push(interface);
                }
            }
        }
        result
    }

    pub(super) fn concrete_implementation_applies(
        &mut self,
        condition: &export::ConditionalInterfaceApplication,
        substitution: &[concrete::TypeId],
    ) -> bool {
        let arguments = condition
            .arguments
            .iter()
            .map(|&argument| self.lower_type(argument, substitution))
            .collect::<Vec<_>>();
        let declaration = self
            .source
            .conditional_interfaces(condition.owner)
            .iter()
            .find(|declaration| declaration.id() == condition.declaration)
            .expect("a conformance references its original conditional declaration");
        self.concrete_interface_condition_holds(declaration, &arguments)
    }

    pub(super) fn concrete_interface_condition_holds(
        &mut self,
        declaration: &export::ConditionalInterfaceDeclaration,
        arguments: &[concrete::TypeId],
    ) -> bool {
        let key = (declaration.id(), arguments.to_vec());
        if let Some(&result) = self.conditional_interface_results.get(&key) {
            return result;
        }
        let result = self.concrete_conditional_parameters_hold(&declaration.parameters, arguments);
        self.conditional_interface_results.insert(key, result);
        result
    }

    pub(super) fn concrete_conditional_parameters_hold(
        &mut self,
        parameters: &[export::TypeParamDecl],
        arguments: &[concrete::TypeId],
    ) -> bool {
        let mut companions = Vec::with_capacity(parameters.len());
        for (parameter, &argument) in parameters.iter().zip(arguments) {
            let companion = if parameter.companion.is_some() {
                let Some(companion) = self.public_companion_type(argument) else {
                    return false;
                };
                Some(companion)
            } else {
                None
            };
            companions.push(companion);
        }
        for ((parameter, &argument), companion) in parameters.iter().zip(arguments).zip(companions)
        {
            if !self.concrete_parameter_bounds_hold(argument, &parameter.bounds, arguments) {
                return false;
            }
            for excluded in &parameter.excluded_interfaces {
                let required = self.lower_type(excluded.ty, arguments);
                if self.concrete_subtype(argument, required) {
                    return false;
                }
            }
            if let (Some(bounds), Some(companion)) = (&parameter.companion, companion) {
                if !self.concrete_parameter_bounds_hold(companion, &bounds.bounds, arguments) {
                    return false;
                }
                for excluded in &bounds.excluded_interfaces {
                    let required = self.lower_type(excluded.ty, arguments);
                    if self.concrete_subtype(companion, required) {
                        return false;
                    }
                }
            }
        }
        true
    }

    fn concrete_parameter_bounds_hold(
        &mut self,
        argument: concrete::TypeId,
        bounds: &export::TypeParamBounds,
        substitution: &[concrete::TypeId],
    ) -> bool {
        match bounds {
            export::TypeParamBounds::Unconstrained => true,
            export::TypeParamBounds::Value { .. } => self.is_value_representation(argument),
            export::TypeParamBounds::Ref { .. } => !self.is_value_representation(argument),
            export::TypeParamBounds::Nominal(bounds) => {
                bounds.in_source_order().into_iter().all(|bound| {
                    let required = self.lower_type(bound.ty(), substitution);
                    self.concrete_subtype(argument, required)
                })
            }
        }
    }

    fn public_companion_type(&mut self, host: concrete::TypeId) -> Option<concrete::TypeId> {
        let owner = self.type_nominal(host)?;
        let local = self.source.nominal_identities.declaration(owner);
        if let Some((_, relation)) = self
            .source
            .companion_relations
            .iter()
            .find(|(_, relation)| Some(relation.host) == local)
        {
            if self.source.objects[relation.object].access.declared
                != export::DeclaredVisibility::Public
            {
                return None;
            }
            return Some(self.lower_companion_type(host));
        }
        let parent = match owner {
            export::SourceNominalId::Concrete(id) => scoop_identity::DefinitionOwnerAtom::Type(id),
            export::SourceNominalId::GenericTemplate(id) => {
                scoop_identity::DefinitionOwnerAtom::GenericType(id)
            }
        };
        let exists = self.source.loaded_class_definitions.values().any(|definition| {
            matches!(definition.declaration.interface.source_shape(), export::NominalSourceShapeV1::Object(shape)
                if shape.object_kind() == export::ObjectSourceKindV1::Companion)
            && definition.declaration.identity.declaration().owners().owners().last() == Some(&parent)
            && definition.declaration.interface.declaration_details().declared_visibility() == export::DeclaredVisibilityV1::Public
        });
        exists.then(|| self.lower_companion_type(host))
    }
}
