use super::*;

impl Lowerer {
    #[cfg(test)]
    pub(super) fn resolve_source_signatures(
        &mut self,
        declarations: &SourceDeclarations<'_>,
    ) -> Result<SignatureCore, Vec<Diagnostic>> {
        self.resolve_source_signatures_with_cancellation(declarations, &cancellation::uninterrupted)
            .map_err(PhaseFailure::into_diagnostics)
    }

    pub(super) fn resolve_source_signatures_with_cancellation<C>(
        &mut self,
        declarations: &SourceDeclarations<'_>,
        check: &impl Fn() -> Result<(), C>,
    ) -> Result<SignatureCore, PhaseFailure<C>> {
        PhaseFailure::check(check)?;
        let files = &declarations.source_spans;
        let SourceDeclarations {
            pending_structs,
            pending_enums,
            pending_classes,
            pending_interfaces,
            pending_objects,
            function_headers,
            pending_functions,
            pending_globals,
            method_headers,
            pending_methods,
            ..
        } = declarations;
        let defines_core = matches!(self.core, CoreLoweringAuthority::Defined);
        let primary_output_file = self.primary_output_file();
        let core_diagnostic_file = self.core_diagnostic_file();
        // Alias targets may mention any declaration in the Cone, including a
        // later alias, `Option<T>` through nullable syntax, and the special
        // pointer families. Establish those source identities before the
        // alias graph is expanded. Application bounds are checked again once
        // every nominal constraint is complete below.
        if defines_core {
            self.ffi_ptr = self.require_core_struct("Ptr", files);
            self.ffi_fun_ptr = self.require_core_struct("FunPtr", files);
            self.ffi_pinned_ptr = self.require_core_struct("PinnedPtr", files);
            self.ffi_gc_handle = self.require_core_struct("GcHandle", files);
            self.ffi_foreign_callback = self.require_core_struct("ForeignCallback", files);
            self.validate_option_enum(files);
        }
        self.deferred_type_bounds = Some(Vec::new());
        PhaseFailure::check(check)?;
        self.resolve_all_type_aliases();
        PhaseFailure::check(check)?;

        self.resolve_nominal_parameter_constraints_with_cancellation(
            pending_structs,
            pending_enums,
            pending_classes,
            pending_interfaces,
            pending_objects,
            check,
        )
        .map_err(PhaseFailure::Cancelled)?;
        PhaseFailure::check(check)?;
        self.validate_nominal_type_parameter_constraints();
        let intrinsic_type_core = if defines_core {
            self.validate_intrinsic_type_core(files)
        } else {
            None
        };
        self.defined_intrinsic_type_core = intrinsic_type_core;
        if let Some(core) = intrinsic_type_core {
            if self.ffi_ptr != Some(core.ptr) {
                self.current_file = primary_output_file;
                self.error(
                    files[core_diagnostic_file],
                    "the `Ptr` FFI core owner must be the `core_ptr` intrinsic type".to_string(),
                );
            }
            if self.ffi_fun_ptr != Some(core.fun_ptr) {
                self.current_file = primary_output_file;
                self.error(
                    files[core_diagnostic_file],
                    "the `FunPtr` FFI core owner must be the `core_fun_ptr` intrinsic type"
                        .to_string(),
                );
            }
        }
        for &(id, decl, file_index) in pending_interfaces {
            PhaseFailure::check(check)?;
            self.current_file = file_index;
            self.current_owner = Some(Owner::Interface(id));
            let declaration_check = self.begin_editing_declaration(Owner::Interface(id));
            self.type_params_in_scope = self.interfaces[id].type_params.clone();
            let parents = self.resolve_supertype_interface_list(&decl.supertypes);
            self.type_params_in_scope.clear();
            self.interfaces[id].parents = parents;
            self.finish_editing_declaration(declaration_check);
        }
        self.current_owner = None;
        if !self.check_interface_inheritance_cycles(pending_interfaces)
            && self.editing_declarations.is_none()
        {
            return Err(self.take_source_diagnostics().into());
        }

        // The core library's `Option<T>` must be validated before any
        // type annotation is resolved: `T?` desugars to it (spec 7.1).
        // It was validated before alias expansion above because an alias
        // target may itself contain nullable syntax.
        // The core library's `Throwable` is the root every `throw`
        // operand and catch parameter type is checked against (spec
        // 11.7).
        if defines_core {
            self.validate_throwable(files);
        }

        for &(id, decl, file_index) in pending_interfaces {
            PhaseFailure::check(check)?;
            self.current_file = file_index;
            self.current_owner = Some(Owner::Interface(id));
            let declaration_check = self.begin_editing_declaration(Owner::Interface(id));
            self.resolve_interface_properties(id, decl);
            self.finish_editing_declaration(declaration_check);
        }
        self.current_owner = None;

        // Pass 2: resolve struct fields, enum variants, class
        // constructor properties and inheritance clauses (all type
        // names are known now, so fields may reference later-declared
        // types).
        for &(id, decl, file_index) in pending_structs {
            PhaseFailure::check(check)?;
            self.current_file = file_index;
            self.current_owner = Some(Owner::Struct(id));
            let declaration_check = self.begin_editing_declaration(Owner::Struct(id));
            self.allow_deferred_fun_ptr = Some(id) == self.ffi_foreign_callback;
            self.resolve_fields(id, decl);
            self.allow_deferred_fun_ptr = false;
            self.type_params_in_scope = self.structs[id].type_params.clone();
            let interfaces = self.resolve_supertype_interface_list(&decl.supertypes);
            self.type_params_in_scope.clear();
            self.structs[id].interfaces = interfaces;
            self.finish_editing_declaration(declaration_check);
        }
        for &(id, decl, file_index) in pending_enums {
            PhaseFailure::check(check)?;
            self.current_file = file_index;
            self.current_owner = Some(Owner::Enum(id));
            let declaration_check = self.begin_editing_declaration(Owner::Enum(id));
            self.resolve_enum_supertypes(id, decl);
            self.resolve_variants(id, decl);
            self.finish_editing_declaration(declaration_check);
        }
        if let Err(error) = self.establish_enum_member_identities() {
            self.current_file = error.file();
            self.error(error.span(), error.to_string());
            return Err(self.take_source_diagnostics().into());
        }
        if defines_core {
            self.validate_option_variants();
        }
        for (id, decl, file_index) in pending_classes {
            PhaseFailure::check(check)?;
            self.current_file = *file_index;
            self.current_owner = Some(Owner::Class(*id));
            let declaration_check = self.begin_editing_declaration(Owner::Class(*id));
            self.resolve_class(*id, *decl);
            self.finish_editing_declaration(declaration_check);
        }
        for &(id, decl, file_index) in pending_objects {
            PhaseFailure::check(check)?;
            self.current_file = file_index;
            self.current_owner = Some(Owner::Object(id));
            let declaration_check = self.begin_editing_declaration(Owner::Object(id));
            self.resolve_object(id, decl);
            self.finish_editing_declaration(declaration_check);
        }
        self.current_owner = None;

        if !self.check_class_inheritance_cycles(pending_classes, pending_objects)
            && self.editing_declarations.is_none()
        {
            return Err(self.take_source_diagnostics().into());
        }
        self.validate_deferred_type_bounds();

        // Inline value layout must be finite before signatures or bodies can
        // request concrete applications. The declaration graph deliberately
        // stops at every reference/pointer boundary and recognizes generic
        // growth by template identity rather than materializing applications.
        if !self.validate_value_layout_cycles() && self.editing_declarations.is_none() {
            // No later pass may try to materialize an application whose
            // inline layout grows forever. The validator has collected one
            // stable definition-site diagnostic for every cyclic SCC.
            return Err(self.take_source_diagnostics().into());
        }

        // Every nominal constraint and inheritance edge is now complete, so
        // fixed alias applications can prove both kind and nominal bounds.
        // Intrinsic owner lookup is also total, which lets exposure witnesses
        // retain the real core declaration domain for primitive targets.
        PhaseFailure::check(check)?;
        self.validate_type_alias_targets();
        PhaseFailure::check(check)?;

        // Pass 2.5: resolve function and method signatures, so calls
        // in any body see parameter and return types regardless of
        // declaration order. Interface method signatures become
        // `hir::MethodSig`s; bodyless declarations (interface and
        // abstract methods) get their parameter-only body here.
        for &(id, decl, file_index) in function_headers {
            PhaseFailure::check(check)?;
            self.current_file = file_index;
            self.current_owner = None;
            self.resolve_function_header(id, decl);
        }
        for &(id, decl, file_index, owner) in method_headers {
            PhaseFailure::check(check)?;
            self.current_file = file_index;
            self.current_owner = Some(owner);
            self.resolve_method_header(id, decl, owner);
        }
        for &(id, declaration, _) in pending_functions {
            PhaseFailure::check(check)?;
            self.register_inferred_function(id, declaration);
        }
        for &(id, declaration, _, _) in pending_methods {
            PhaseFailure::check(check)?;
            self.register_inferred_function(id, declaration);
        }
        self.current_owner = None;

        if defines_core {
            self.validate_core_operator_intrinsics(files);
            self.validate_array_conversion_intrinsics(files);
            self.validate_gc_control_intrinsics(files);
            self.validate_string_intrinsics(files);
            self.validate_data_borrow_intrinsics();
            self.validate_atomic_intrinsics(files);
            self.validate_maybe_uninit_intrinsics();
        }
        let source_location_core = if defines_core {
            self.validate_source_location_core(files)
        } else {
            None
        };
        let iteration_core = if defines_core {
            self.validate_iteration_core(files)
        } else {
            None
        };
        self.iteration_core = iteration_core;

        // M10's coroutine protocol is compiler-known: MIR generation needs
        // these exact generic interfaces and intrinsic signatures rather than
        // guessing entities from names after HIR.
        let coroutine_core = if defines_core {
            self.validate_coroutine_core(files)
        } else {
            None
        };
        let ffi_core = if defines_core {
            self.validate_ffi_core(files)
        } else {
            None
        };
        self.ffi_core = ffi_core;
        let foreign_callback_core = if defines_core {
            self.validate_foreign_callback_core(files)
        } else {
            None
        };
        self.foreign_callback_core = foreign_callback_core;
        PhaseFailure::check(check)?;
        self.resolve_globals(pending_globals, pending_objects, pending_enums);
        PhaseFailure::check(check)?;
        self.finalize_import_targets();
        self.resolve_property_accessor_signatures();
        self.check_extension_property_signatures();

        // Publish the complete duplicate-signature rejection set after all
        // related signatures are final and before body-capable passes may
        // perform semantic callable lookup.
        self.validate_and_freeze_duplicate_signatures_with_cancellation(
            function_headers,
            method_headers,
            check,
        )
        .map_err(PhaseFailure::Cancelled)?;
        assert!(
            self.declaration_surface.is_frozen(),
            "body-capable passes require a frozen declaration surface"
        );

        if !self.resolve_errno_annotations(function_headers, pending_globals)
            && self.editing_declarations.is_none()
        {
            return Err(self.take_source_diagnostics().into());
        }
        self.validate_extern_functions();
        self.validate_extern_global_symbols();

        PhaseFailure::check(check)?;
        self.resolve_annotation_declarations();
        PhaseFailure::check(check)?;

        // Pass 2.75: inheritance checks (milestone6 DESIGN.md 2.2) —
        // cycles, property shadowing, override rules and interface
        // implementation (classes and value types alike). Needs every
        // signature and inheritance clause.
        self.check_inheritance(
            pending_classes,
            pending_structs,
            pending_enums,
            pending_interfaces,
            pending_objects,
            method_headers,
        );
        // Defaults and constructor expressions can select derived equality.
        // Publish its conditional signatures before any of those bodies lower.
        self.declare_derived_equality_methods();
        self.register_export_parameter_interfaces(
            function_headers,
            method_headers,
            pending_structs,
            pending_classes,
            pending_enums,
        );
        self.register_object_parameter_interfaces(
            &pending_objects
                .iter()
                .map(|&(id, _, _)| id)
                .collect::<Vec<_>>(),
        );

        PhaseFailure::check(check)?;
        self.register_derived_parameter_interfaces();
        PhaseFailure::check(check)?;
        Ok(SignatureCore {
            intrinsic_type_core,
            source_location_core,
            iteration_core,
            coroutine_core,
            ffi_core,
            foreign_callback_core,
        })
    }
}
