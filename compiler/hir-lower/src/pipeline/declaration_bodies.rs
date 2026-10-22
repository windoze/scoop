use super::*;

impl Lowerer {
    pub(super) fn complete_declaration_bodies(
        &mut self,
        declarations: &SourceDeclarations<'_>,
    ) -> Result<DeclarationBodyCore, Vec<Diagnostic>> {
        let files = &declarations.source_headers;
        let SourceDeclarations {
            pending_structs,
            pending_enums,
            pending_classes,
            pending_interfaces,
            pending_objects,
            function_headers,
            pending_globals,
            method_headers,
            ..
        } = declarations;
        let defines_core = matches!(self.core, CoreLoweringAuthority::Defined);
        self.declaration_inference.ready = true;
        self.complete_inferred_functions();
        self.complete_inferred_properties();
        if self.declaration_inference_failed() {
            return Err(self.take_source_diagnostics());
        }
        let raw_initializers = std::mem::take(&mut self.pending_raw_initializers);
        self.resolve_raw_global_initializers(raw_initializers);
        self.validate_inferred_result_contracts();
        self.validate_signature_exposure();
        self.finish_export_parameter_interfaces();
        self.resolve_constructor_graphs(pending_classes, pending_structs, pending_objects);
        self.lower_constructor_initialization(pending_classes, pending_structs, pending_objects);
        for &(id, declaration, file) in pending_structs {
            self.annotate_struct(id, declaration, file);
        }
        for &(id, declaration, file) in pending_enums {
            self.annotate_enum(id, declaration, file);
        }
        for &(id, declaration, file) in pending_classes {
            self.annotate_class(id, declaration, file);
        }
        for &(id, declaration, file) in pending_interfaces {
            self.annotate_interface(id, declaration, file);
        }
        for &(id, declaration, file) in pending_objects {
            self.annotate_object(id, declaration, file);
        }
        self.current_owner = None;

        for &(id, declaration, file) in function_headers {
            self.current_file = file;
            self.current_owner = None;
            self.type_params_in_scope = self.signatures[&id].type_params.clone();
            self.annotate_function(id, declaration);
        }
        for &(id, declaration, file, owner) in method_headers {
            self.current_file = file;
            self.current_owner = Some(owner);
            self.type_params_in_scope = self.signatures[&id].type_params.clone();
            self.annotate_function(id, declaration);
        }
        for &(declaration, file) in pending_globals {
            self.current_file = file;
            self.current_owner = None;
            self.type_params_in_scope.clear();
            self.annotate_property_declaration(declaration, file);
        }
        self.annotate_type_aliases();
        self.current_owner = None;
        self.type_params_in_scope.clear();

        // Compiler-generated exception edges receive complete typed class /
        // zero-argument-constructor identities after inheritance has been
        // validated and before body lowering. MIR never recovers these
        // targets from names.
        let program_arguments = if defines_core {
            self.validate_program_arguments()
        } else {
            None
        };
        let type_reference = if defines_core {
            self.validate_type_info_core(files)
        } else {
            None
        };
        let exception_core = if defines_core {
            self.validate_exception_core(files)
        } else {
            None
        };
        if defines_core && (self.option_core.is_none() || exception_core.is_none()) {
            return Err(self.take_source_diagnostics());
        }
        self.lower_runtime_top_level_initializers();

        Ok(DeclarationBodyCore {
            program_arguments,
            type_reference,
            exception_core,
        })
    }
}
