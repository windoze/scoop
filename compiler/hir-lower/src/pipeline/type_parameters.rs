use super::*;

impl Lowerer {
    pub(super) fn resolve_nominal_parameter_constraints(
        &mut self,
        pending_structs: &[(StructId, &ast::StructDecl, usize)],
        pending_enums: &[(EnumId, &ast::EnumDecl, usize)],
        pending_classes: &[(ClassId, &ast::ClassDecl, usize)],
        pending_interfaces: &[(InterfaceId, &ast::InterfaceDecl, usize)],
        pending_objects: &[(ObjectId, crate::declarations::ObjectSource<'_>, usize)],
    ) {
        // Type-parameter names and arities are declared in pass 1. Resolve
        // their ordered constraints only after every nominal name is visible,
        // then validate bound applications after all constraint sets are
        // complete (F-bounds may form legal dependency cycles).
        for &(id, decl, file_index) in pending_structs {
            self.current_file = file_index;
            self.current_owner = Some(Owner::Struct(id));
            let declared = self.structs[id].type_params.clone();
            let params = self.resolve_type_parameter_constraints(
                declared,
                0,
                &decl.type_params,
                decl.where_clause.as_ref(),
                "struct",
            );
            self.structs[id].type_params = params;
        }
        for &(id, decl, file_index) in pending_enums {
            self.current_file = file_index;
            self.current_owner = Some(Owner::Enum(id));
            let declared = self.enums[id].type_params.clone();
            let params = self.resolve_type_parameter_constraints(
                declared,
                0,
                &decl.type_params,
                decl.where_clause.as_ref(),
                "enum",
            );
            self.enums[id].type_params = params;
        }
        for &(id, decl, file_index) in pending_classes {
            self.current_file = file_index;
            self.current_owner = Some(Owner::Class(id));
            let declared = self.classes[id].type_params.clone();
            let params = self.resolve_type_parameter_constraints(
                declared,
                0,
                &decl.type_params,
                decl.where_clause.as_ref(),
                "class",
            );
            self.classes[id].type_params = params;
        }
        for &(id, decl, file_index) in pending_interfaces {
            self.current_file = file_index;
            self.current_owner = Some(Owner::Interface(id));
            let declared = self.interfaces[id].type_params.clone();
            let params = self.resolve_type_parameter_constraints(
                declared,
                0,
                &decl.type_params,
                decl.where_clause.as_ref(),
                "interface",
            );
            self.interfaces[id].type_params = params;
        }
        for &(object, _, _) in pending_objects {
            self.complete_companion_parameters(object);
        }
        self.current_owner = None;
    }

    fn complete_companion_parameters(&mut self, object: ObjectId) {
        let Some(host) = self.companion_host(object) else {
            return;
        };
        let parameters = self.owner_type_params(host);
        if parameters.is_empty() {
            return;
        }
        let declaration = &self.objects[object];
        self.classes[declaration.backing_class].type_params = parameters.clone();
        let unit = &self.initialization_units
            [self.singleton_values[declaration.singleton_value].initialization];
        for function in [unit.initializer, unit.ensure] {
            self.signatures
                .get_mut(&function)
                .expect("singleton initialization has a declared signature")
                .type_params = parameters.clone();
            let hir::FunctionGenericity::Generic {
                parameters: inherited,
                ..
            } = &mut self.functions[function].genericity
            else {
                unreachable!(
                    "generic companion initialization retains its declared generic identity"
                )
            };
            inherited.clone_from(&parameters);
        }
    }
}
