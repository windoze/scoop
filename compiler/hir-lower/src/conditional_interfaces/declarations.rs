use super::*;

impl Lowerer {
    pub(crate) fn resolve_pending_conditional_interfaces(&mut self) {
        let pending = std::mem::take(&mut self.conditional_interfaces.pending);
        let saved_owner = self.current_owner;
        let saved_file = self.current_file;
        let saved_parameters = std::mem::take(&mut self.type_params_in_scope);
        for implementation in pending {
            self.current_owner = Some(implementation.owner);
            self.current_file = implementation.file;
            self.resolve_interface_block(implementation);
        }
        self.current_owner = saved_owner;
        self.current_file = saved_file;
        self.type_params_in_scope = saved_parameters;
    }

    fn resolve_interface_block(&mut self, pending: PendingImplementation) {
        let owner = pending.owner;
        let declaration = pending.declaration;
        let intrinsic = match owner {
            Owner::Class(id) => !self.classes[id].is_declared(),
            Owner::Struct(id) => matches!(
                self.structs[id].representation,
                hir::StructRepresentation::Intrinsic(_)
            ),
            Owner::Object(_) | Owner::Enum(_) => false,
            Owner::Interface(_) => {
                unreachable!("interface bodies cannot declare implementation blocks")
            }
        };
        if intrinsic {
            self.error(
                declaration.span,
                "intrinsic type bodies cannot declare conditional interface implementations"
                    .to_string(),
            );
            return;
        }
        let parameters = self.resolve_type_parameter_constraints(
            self.owner_type_params(owner),
            0,
            &[],
            Some(&declaration.where_clause),
            true,
            "interface implementation",
        );
        self.type_params_in_scope = parameters.clone();
        let Some(interface) = self.resolve_type_ref(&declaration.interface) else {
            return;
        };
        if !matches!(self.types[interface], Type::Interface(_)) {
            self.error(
                declaration.interface.span,
                "an interface implementation must target an interface type".to_string(),
            );
            return;
        }
        let Some(key) =
            self.conditional_interface_key(owner, interface, &parameters, declaration.span)
        else {
            return;
        };
        let identity = key.id();
        let resolved = hir::ConditionalInterfaceDeclaration {
            key,
            interface,
            parameters,
            span: declaration.span,
        };
        self.check_conditional_interface_overlap(owner, &resolved);
        match owner {
            Owner::Class(id) => self.classes[id].conditional_interfaces.push(resolved),
            Owner::Object(id) => self.classes[self.objects[id].backing_class]
                .conditional_interfaces
                .push(resolved),
            Owner::Struct(id) => self.structs[id].conditional_interfaces.push(resolved),
            Owner::Enum(id) => self.enums[id].conditional_interfaces.push(resolved),
            Owner::Interface(_) => unreachable!("implementation owners are ordinary types"),
        }
        for function in pending.functions {
            self.functions[function]
                .method
                .as_mut()
                .expect("implementation functions are methods")
                .availability = Some(identity);
        }
        for property in declaration.properties() {
            self.resolve_conditional_property(owner, identity, property);
        }
    }
}
