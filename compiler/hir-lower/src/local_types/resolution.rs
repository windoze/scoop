use super::*;
use declarations::Declarations;

impl Lowerer {
    pub(super) fn resolve_local_type_declarations(&mut self, pending: &Declarations<'_>) {
        let errors = self.diagnostics.len();
        let accessor_start = self.property_accessor_sources.len();
        let function_start = self.functions.len();
        let property_start = self.properties.len();
        let default_start = self.pending_default_interfaces();
        self.deferred_type_bounds = Some(Vec::new());
        self.resolve_nominal_parameter_constraints(
            &pending.structs,
            &pending.enums,
            &pending.classes,
            &pending.interfaces,
            &pending.objects,
        );
        for &(id, declaration, file) in &pending.interfaces {
            self.current_file = file;
            self.current_owner = Some(Owner::Interface(id));
            self.type_params_in_scope = self.interfaces[id].type_params.clone();
            self.interfaces[id].parents =
                self.resolve_supertype_interface_list(&declaration.supertypes);
        }
        self.validate_owner_type_parameter_constraints(&pending.owners());
        self.resolve_local_owner_aliases(&pending.owners());
        if !self.check_interface_inheritance_cycles(&pending.interfaces) {
            self.deferred_type_bounds = None;
            return;
        }
        for &(id, declaration, file) in &pending.interfaces {
            self.current_file = file;
            self.current_owner = Some(Owner::Interface(id));
            self.resolve_interface_properties(id, declaration);
        }
        for &(id, declaration, file) in &pending.structs {
            self.current_file = file;
            self.current_owner = Some(Owner::Struct(id));
            self.resolve_fields(id, declaration);
            self.type_params_in_scope = self.structs[id].type_params.clone();
            self.structs[id].interfaces =
                self.resolve_supertype_interface_list(&declaration.supertypes);
        }
        for &(id, declaration, file) in &pending.enums {
            self.current_file = file;
            self.current_owner = Some(Owner::Enum(id));
            self.resolve_enum_supertypes(id, declaration);
            self.resolve_variants(id, declaration);
        }
        self.resolve_local_integral_enum_constants(&pending.enums);
        if let Err(error) = self.establish_enum_member_identities() {
            self.current_file = error.file();
            self.error(error.span(), error.to_string());
            return;
        }
        for &(id, declaration, file) in &pending.classes {
            self.current_file = file;
            self.current_owner = Some(Owner::Class(id));
            self.resolve_class(id, declaration);
        }
        for &(id, declaration, file) in &pending.objects {
            self.current_file = file;
            self.current_owner = Some(Owner::Object(id));
            self.resolve_object(id, declaration);
        }
        self.current_owner = None;
        self.type_params_in_scope.clear();
        if !self.check_class_inheritance_cycles(&pending.classes, &pending.objects) {
            self.deferred_type_bounds = None;
            return;
        }
        self.validate_deferred_type_bounds();
        if !self.validate_local_value_layouts(&pending.owners()) {
            return;
        }
        for &(id, declaration, file, owner) in &pending.methods {
            self.current_file = file;
            self.current_owner = Some(owner);
            self.resolve_method_signature(id, declaration, owner);
        }
        let method_headers = pending
            .methods
            .iter()
            .map(|&(id, declaration, file, owner)| (id, declaration.header(), file, owner))
            .collect::<Vec<_>>();
        self.resolve_property_accessor_signatures_from(accessor_start);
        self.validate_and_freeze_duplicate_signatures(&[], &method_headers);
        self.resolve_local_owner_annotations(&pending.owners());
        self.annotate_local_owner_aliases(&pending.owners());
        self.check_inheritance(
            &pending.classes,
            &pending.structs,
            &pending.enums,
            &pending.interfaces,
            &pending.objects,
            &method_headers,
        );
        self.declare_owner_equality_methods(&pending.owners());
        self.register_export_parameter_interfaces(
            &[],
            &method_headers,
            &pending.structs,
            &pending.classes,
            &pending.enums,
        );
        self.register_object_parameter_interfaces(
            &pending
                .objects
                .iter()
                .map(|&(id, _, _)| id)
                .collect::<Vec<_>>(),
        );
        self.register_derived_parameter_interfaces_from(function_start);
        if self.diagnostics.len() != errors {
            return;
        }
        let properties = self
            .properties
            .iter()
            .skip(property_start)
            .map(|(id, _)| id)
            .collect::<Vec<_>>();
        for id in properties {
            self.infer_property_type(id);
        }
        for &(id, _, _, _) in &pending.methods {
            if self.has_inferred_function(id) {
                self.infer_function_result(id);
            }
        }
        self.validate_inferred_result_contracts();
        self.finish_export_parameter_interfaces_from(default_start);
        self.resolve_constructor_graphs(&pending.classes, &pending.structs, &pending.objects);
        self.lower_constructor_initialization(&pending.classes, &pending.structs, &pending.objects);
        for &(id, declaration, file) in &pending.structs {
            self.annotate_struct(id, declaration, file);
        }
        for &(id, declaration, file) in &pending.enums {
            self.annotate_enum(id, declaration, file);
        }
        for &(id, declaration, file) in &pending.classes {
            self.annotate_class(id, declaration, file);
        }
        for &(id, declaration, file) in &pending.interfaces {
            self.annotate_interface(id, declaration, file);
        }
        for &(id, declaration, file) in &pending.objects {
            self.annotate_object(id, declaration, file);
        }
        for &(id, declaration, file, owner) in &pending.methods {
            self.current_file = file;
            self.current_owner = Some(owner);
            self.type_params_in_scope = self.signatures[&id].type_params.clone();
            self.annotate_function(id, declaration.header());
        }

        for &(id, declaration, file, _) in &pending.methods {
            if self.has_inferred_function(id) || matches!(declaration.body, ast::FunctionBody::None)
            {
                continue;
            }
            self.current_file = file;
            let body = self.lower_body(id, declaration);
            self.functions[id].kind = hir::FunctionKind::User(body);
        }
        self.lower_interface_forwarder_bodies_from(function_start);
        self.lower_property_accessor_bodies_from(accessor_start);
        self.lower_release_blocks(&pending.classes, &pending.objects);
    }
}
