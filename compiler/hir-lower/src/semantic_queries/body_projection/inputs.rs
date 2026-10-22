use super::*;

impl ProjectionData {
    pub(super) fn inputs<'a>(&'a self, lowerer: &'a Lowerer) -> hir::HirBodyProjectionInputs<'a> {
        hir::HirBodyProjectionInputs {
            extern_functions: &lowerer.extern_functions,
            export_default_exprs: &lowerer.export_default_exprs,
            export_default_sources: &lowerer.export_default_sources,
            type_inputs: crate::persistent_types::identity_inputs(
                lowerer,
                lowerer
                    .nominal_identities
                    .as_ref()
                    .expect("completed nominals"),
                lowerer.query_core_types(),
            ),
            sources: hir::HirSourceProjectionInputs {
                source_files: &self.source_files,
                source_contexts: &lowerer.source_contexts,
                source_context_identities: &self.contexts,
            },
            anonymous_functions: lowerer.anonymous_functions.as_arena(),
            bound_callable_refs: &lowerer.bound_callable_refs,
            callable_references: &lowerer.callable_references,
            callback_registration_identities: &self.callbacks,
            class_applications: &lowerer.class_applications,
            class_constructor_applications: &lowerer.class_constructor_applications,
            class_constructors: lowerer.class_constructors.as_arena(),
            constructor_identities: &self.constructors,
            default_local_value_scopes: &self.local_scopes,
            derived_equality_applications: lowerer.derived_equality_applications.as_arena(),
            enum_applications: &lowerer.enum_applications,
            enum_member_identities: lowerer
                .enum_member_identities
                .as_ref()
                .expect("completed variants"),
            foreign_callback_registrations: &lowerer.foreign_callback_registrations,
            function_coercions: &lowerer.function_coercions,
            function_identities: &self.functions,
            function_types: &lowerer.function_types,
            functions: lowerer.functions.as_arena(),
            generic_delegate_templates: &lowerer.generic_delegate_templates,
            generic_functions: &lowerer.generic_functions,
            generic_method_applications: &lowerer.generic_method_applications,
            generic_methods: &lowerer.generic_methods,
            globals: &lowerer.globals,
            imported_signatures: lowerer,
            imported_dependency_callables: &lowerer.imported_dependency_callables,
            imported_generic_applications: lowerer.imported_generic_applications.as_arena(),
            imported_generic_delegate_templates: &lowerer.imported_generic_delegate_templates,
            initialization_unit_identities: &self.initializations,
            initialization_units: &lowerer.initialization_units,
            instantiations: lowerer.instantiations.as_arena(),
            interface_applications: &lowerer.interface_applications,
            lambdas: lowerer.lambdas.as_arena(),
            local_functions: lowerer.local_functions.as_arena(),
            member_requirement_implementations: &lowerer.member_requirement_implementations,
            member_requirement_owners: &lowerer.member_requirement_owners,
            method_applications: &lowerer.method_applications,
            object_types: &lowerer.object_types,
            object_value_identities: &self.objects,
            property_accessor_identities: &self.accessors,
            property_identities: &self.properties,
            singleton_values: &lowerer.singleton_values,
            struct_applications: &lowerer.struct_applications,
            struct_constructor_applications: &lowerer.struct_constructor_applications,
            struct_constructors: lowerer.struct_constructors.as_arena(),
            types: &lowerer.types,
        }
    }
}

impl hir::ImportedTemplateSignatures for Lowerer {
    fn callable_signature(
        &self,
        template: hir::ImportedGenericCallableTemplateId,
    ) -> &hir::ImportedGenericCallableSignature {
        &self.imported_generic_templates[template].signature
    }

    fn constructor_signature(
        &self,
        template: hir::ImportedConstructorTemplateId,
    ) -> &hir::ImportedConstructorSignature {
        &self.imported_constructor_templates[template].signature
    }
}
