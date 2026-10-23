use super::*;

impl Lowerer {
    pub(crate) fn resolve_nominal_parameter_constraints(
        &mut self,
        pending_structs: &[(StructId, ast::StructSource<'_>, usize)],
        pending_enums: &[(EnumId, ast::EnumSource<'_>, usize)],
        pending_classes: &[(ClassId, ast::ClassSource<'_>, usize)],
        pending_interfaces: &[(InterfaceId, ast::InterfaceSource<'_>, usize)],
        pending_objects: &[(ObjectId, crate::declarations::ObjectSource<'_>, usize)],
    ) {
        self.resolve_nominal_parameter_constraints_with_cancellation(
            pending_structs,
            pending_enums,
            pending_classes,
            pending_interfaces,
            pending_objects,
            &cancellation::uninterrupted,
        )
        .unwrap_or_else(|never| match never {});
    }

    pub(crate) fn resolve_nominal_parameter_constraints_with_cancellation<C>(
        &mut self,
        pending_structs: &[(StructId, ast::StructSource<'_>, usize)],
        pending_enums: &[(EnumId, ast::EnumSource<'_>, usize)],
        pending_classes: &[(ClassId, ast::ClassSource<'_>, usize)],
        pending_interfaces: &[(InterfaceId, ast::InterfaceSource<'_>, usize)],
        pending_objects: &[(ObjectId, crate::declarations::ObjectSource<'_>, usize)],
        check: &impl Fn() -> Result<(), C>,
    ) -> Result<(), C> {
        // Type-parameter names and arities are declared in pass 1. Resolve
        // their ordered constraints only after every nominal name is visible,
        // then validate bound applications after all constraint sets are
        // complete (F-bounds may form legal dependency cycles).
        for &(id, decl, file_index) in pending_structs {
            check()?;
            self.current_file = file_index;
            self.current_owner = Some(Owner::Struct(id));
            let declaration_check = self.begin_editing_declaration(Owner::Struct(id));
            let declared = self.structs[id].type_params.clone();
            let params = self.resolve_type_parameter_constraints(
                declared,
                self.local_type_parameter_count(Owner::Struct(id)),
                &decl.type_params,
                decl.where_clause.as_ref(),
                false,
                "struct",
            );
            self.structs[id].type_params = params;
            self.finish_editing_declaration(declaration_check);
        }
        for &(id, decl, file_index) in pending_enums {
            check()?;
            self.current_file = file_index;
            self.current_owner = Some(Owner::Enum(id));
            let declaration_check = self.begin_editing_declaration(Owner::Enum(id));
            let declared = self.enums[id].type_params.clone();
            let params = self.resolve_type_parameter_constraints(
                declared,
                self.local_type_parameter_count(Owner::Enum(id)),
                &decl.type_params,
                decl.where_clause.as_ref(),
                false,
                "enum",
            );
            self.enums[id].type_params = params;
            self.finish_editing_declaration(declaration_check);
        }
        for &(id, decl, file_index) in pending_classes {
            check()?;
            self.current_file = file_index;
            self.current_owner = Some(Owner::Class(id));
            let declaration_check = self.begin_editing_declaration(Owner::Class(id));
            let mut declared = self.classes[id].type_params.clone();
            let outer_count = self
                .enclosing_parameter_count(id)
                .max(self.local_type_parameter_count(Owner::Class(id)));
            if decl.inner_span.is_some()
                && let Some(hir::NominalOwner::Class(outer)) = self.classes[id].owner
            {
                declared[..outer_count].clone_from_slice(&self.classes[outer].type_params);
            }
            let params = self.resolve_type_parameter_constraints(
                declared,
                outer_count,
                &decl.type_params,
                decl.where_clause.as_ref(),
                false,
                "class",
            );
            self.classes[id].type_params = params;
            self.finish_editing_declaration(declaration_check);
        }
        for &(id, decl, file_index) in pending_interfaces {
            check()?;
            self.current_file = file_index;
            self.current_owner = Some(Owner::Interface(id));
            let declaration_check = self.begin_editing_declaration(Owner::Interface(id));
            let declared = self.interfaces[id].type_params.clone();
            let params = self.resolve_type_parameter_constraints(
                declared,
                self.local_type_parameter_count(Owner::Interface(id)),
                &decl.type_params,
                decl.where_clause.as_ref(),
                false,
                "interface",
            );
            self.interfaces[id].type_params = params;
            self.finish_editing_declaration(declaration_check);
        }
        for &(object, _, _) in pending_objects {
            check()?;
            self.complete_companion_parameters(object);
        }
        self.current_owner = None;
        Ok(())
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
