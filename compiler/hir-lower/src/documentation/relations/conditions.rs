use super::*;

impl Lowerer {
    pub(super) fn documentation_property_condition(
        &mut self,
        property: hir::PropertyId,
        application: hir::MethodOwnerApplication,
    ) -> Option<hir::ConditionalInterfaceApplication> {
        let getter = self.properties[property].capability.getter();
        let function = accessor_function(self.property_getters[getter].implementation)?;
        let arguments = self.method_owner_arguments(application).to_vec();
        self.local_member_condition(function, &arguments)
    }

    pub(in crate::documentation) fn document_member_condition(
        &mut self,
        declaration: &mut hir::DocumentationDeclaration,
    ) {
        let function = match declaration.target {
            Target::Declaration(Source::Callable(callable)) => match callable {
                Callable::Function(function) => Some(function),
                Callable::Getter(getter) => {
                    accessor_function(self.property_getters[getter].implementation)
                }
                Callable::Setter(setter) => {
                    accessor_function(self.property_setters[setter].implementation)
                }
                _ => None,
            },
            Target::Declaration(Source::Property(property)) => {
                let getter = self.properties[property].capability.getter();
                accessor_function(self.property_getters[getter].implementation)
            }
            Target::DerivedFunction(function) => Some(function),
            _ => None,
        };
        if let Some(guard) = function
            .and_then(|function| self.functions[function].method)
            .and_then(|method| method.availability)
        {
            let condition = self.conditional_guard_application(guard);
            declaration
                .signature
                .push(Part::Text(self.documentation_condition(&condition)));
        }
    }

    pub(super) fn documentation_condition(
        &mut self,
        condition: &hir::ConditionalInterfaceApplication,
    ) -> String {
        let mut parameters = self
            .declaration_conditional_interfaces(condition.owner)
            .iter()
            .find(|declaration| declaration.id() == condition.declaration)
            .expect("a documented member retains its condition")
            .parameters
            .clone();
        let bindings = parameters
            .iter()
            .zip(&condition.arguments)
            .map(|(parameter, &argument)| (parameter.id, argument))
            .collect::<Vec<_>>();
        let scope = self.type_params_in_scope.clone();
        let mut clauses = Vec::new();
        for (parameter, &argument) in parameters.iter_mut().zip(&condition.arguments) {
            self.documentation_substitute_parameter(parameter, &bindings);
            parameter.name = self.type_name_with_params(argument, &scope);
            if !matches!(parameter.bounds, hir::TypeParamBounds::Unconstrained)
                || !parameter.excluded_interfaces.is_empty()
            {
                let clause = crate::call_resolution::diagnostics::render_type_parameters(
                    self,
                    std::slice::from_ref(parameter),
                    &scope,
                );
                clauses.push(clause[1..clause.len() - 1].to_owned());
            }
            let companion = self.documentation_constraints(std::slice::from_ref(parameter), &scope);
            if let Some(companion) = companion.strip_prefix(" where ") {
                clauses.push(companion.to_owned());
            }
        }
        format!(" where {}", clauses.join(", "))
    }

    pub(super) fn documentation_inherited_conditions(
        &mut self,
        ty: hir::TypeId,
    ) -> Vec<hir::ConditionalInterfaceApplication> {
        let mut conditions = Vec::new();
        let mut pending = vec![ty];
        let mut seen = HashSet::new();
        while let Some(ty) = pending.pop() {
            if !seen.insert(ty) {
                continue;
            }
            if let Some(application) = self.nominal_application(ty) {
                for declaration in self.declaration_conditional_interfaces(application.template) {
                    conditions.push(hir::ConditionalInterfaceApplication {
                        declaration: declaration.id(),
                        owner: application.template,
                        arguments: application.arguments.clone(),
                    });
                }
            }
            pending.extend(self.direct_nominal_supertypes(ty));
        }
        conditions
    }
}

fn accessor_function(
    implementation: hir::PropertyAccessorImplementation,
) -> Option<hir::FunctionId> {
    match implementation {
        hir::PropertyAccessorImplementation::Body(function)
        | hir::PropertyAccessorImplementation::StorageBody(function)
        | hir::PropertyAccessorImplementation::AbstractSlot(function) => Some(function),
        hir::PropertyAccessorImplementation::Storage
        | hir::PropertyAccessorImplementation::Constant => None,
    }
}
